use super::{
    movie::{Baseline, Request, FPS, SIZE},
    Gesture, Phase,
};
use crate::export::ExportManager;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tss_live_wheel::{DecodedMediaFrame, LiveWheel, SlideGesture};

pub(super) struct Job {
    pub request: Request,
    pub paths: (PathBuf, PathBuf),
    pub cancel: Arc<AtomicBool>,
}

// A verifier can retain one exact native optical frame beside the finished video.
fn proof_target() -> Result<Option<(u64, PathBuf)>, String> {
    if !std::env::args_os().any(|s| s == "--verify-wheel-export") {
        return Ok(None);
    }
    std::env::var_os("TSS_WHEEL_EXPORT_PROOF")
        .map(|path| {
            let index = std::env::var("TSS_WHEEL_EXPORT_PROOF_FRAME")
                .ok()
                .map(|s| s.parse::<u64>())
                .transpose()
                .map_err(|e| e.to_string())?
                .unwrap_or(0);
            Ok((index, PathBuf::from(path)))
        })
        .transpose()
}
fn write_rgb(path: &std::path::Path, pixels: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut writer =
        std::io::BufWriter::new(std::fs::File::create_new(path).map_err(|e| e.to_string())?);
    write!(writer, "P6\n{} {}\n255\n", SIZE[0], SIZE[1]).map_err(|e| e.to_string())?;
    let mut row = Vec::with_capacity(SIZE[0] as usize * 3);
    for source in pixels.chunks_exact(SIZE[0] as usize * 4) {
        row.clear();
        for pixel in source.chunks_exact(4) {
            row.extend_from_slice(&pixel[..3]);
        }
        writer.write_all(&row).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())
}

pub(crate) fn render(
    r: &mut LiveWheel,
    job: Job,
    last_film_times: [f64; 2],
    app: &tauri::AppHandle,
    mut window_closed: impl FnMut() -> bool,
) -> Result<(), String> {
    let Job {
        request,
        paths: (audio, output),
        cancel,
    } = job;
    let count = request.validate()?;
    let proof = proof_target()?;
    let current_pose = request.baseline == Baseline::Current;
    let [rear_origin, front_origin] = request.baseline.film_origins(last_film_times);
    let mut target = pollster::block_on(r.begin_movie(SIZE, current_pose))?;
    let manager = app.state::<ExportManager>();
    manager.start_rgba(audio, output, SIZE[0], SIZE[1], FPS)?;
    let mut events = request.events.into_iter().peekable();
    let mut held = std::collections::HashMap::new();
    let mut next_report = Instant::now();
    let frames = (|| {
        for index in 0..count {
            if cancel.load(Ordering::SeqCst) || window_closed() {
                return Err("Export cancelled.".to_owned());
            }
            let pts = index as f64 / f64::from(FPS);
            while let Some(e) = events.next_if(|e| e.time <= pts) {
                target.select_gesture_layer(e.layer_id)?;
                let slide = e.slide as u32;
                if e.gesture == Gesture::Pressure {
                    target.set_slide_pressure(slide, e.amount.unwrap_or(0.), e.point)?;
                    continue;
                }
                let previous = held.entry((e.layer_id, slide)).or_insert(None);
                match e.phase {
                    Phase::Down => {
                        target.end_gesture(slide);
                        *previous = Some(e.point);
                    }
                    Phase::Move => {
                        let from = previous
                            .replace(e.point)
                            .ok_or("Missing export gesture anchor")?;
                        target.gesture(
                            slide,
                            match e.gesture {
                                Gesture::Tug => SlideGesture::Tug,
                                Gesture::Rotate => SlideGesture::Rotate,
                                Gesture::Pressure => unreachable!(),
                            },
                            e.tug_response.unwrap_or(0.4),
                            from,
                            e.point,
                        )?;
                    }
                    Phase::Up => {
                        target.end_gesture(slide);
                        *previous = None;
                    }
                }
            }
            target.prepare_media(pts, crate::media_audio::at(pts), |id, us| {
                crate::media::frame_at(id, us).map(|frame| DecodedMediaFrame {
                    id: frame.id,
                    pts_us: frame.pts_us,
                    width: frame.width,
                    height: frame.height,
                    rgba: frame.rgba,
                })
            })?;
            target.frame_films(rear_origin + pts, front_origin + pts, pts, |bytes| {
                if let Some((_, path)) = proof.as_ref().filter(|(n, _)| *n == index) {
                    write_rgb(path, bytes)?;
                }
                manager.append_rgba(index, bytes)
            })?;
            if Instant::now() >= next_report || index + 1 == count {
                let _ = app.emit(
                    "tss-wheel-export-progress",
                    json!({"completed":index+1,"total":count}),
                );
                next_report = Instant::now() + Duration::from_millis(250);
            }
        }
        Ok(())
    })();
    drop(target);
    if let Err(error) = frames {
        manager.cancel()?;
        return Err(error);
    }
    manager.finish_with_cancel(&cancel)
}
