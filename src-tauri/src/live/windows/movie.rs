use super::*;
use crate::export::ExportManager;
use crate::live::movie::{Baseline, Request, FPS, SIZE};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Job {
    pub request: Request,
    paths: (PathBuf, PathBuf),
    cancel: Arc<AtomicBool>,
}
impl Host {
    pub async fn export_movie(
        &self,
        paths: (PathBuf, PathBuf),
        request: Request,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), String> {
        self.request(move |reply| {
            Command::Movie(
                Job {
                    request,
                    paths,
                    cancel,
                },
                reply,
            )
        })
        .await
    }
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

pub(super) fn render(
    r: &mut LiveWheel,
    job: Job,
    last_field_time: f64,
    app: &tauri::AppHandle,
    parent: HWND,
) -> Result<(), String> {
    let Job {
        request,
        paths: (audio, output),
        cancel,
    } = job;
    let count = request.validate()?;
    let proof = proof_target()?;
    let current_pose = request.baseline == Baseline::Current;
    let initial_phase = if current_pose { last_field_time } else { 0. };
    let mut target = pollster::block_on(r.begin_movie(SIZE, current_pose))?;
    let manager = app.state::<ExportManager>();
    manager.start_rgba(audio, output, SIZE[0], SIZE[1], FPS)?;
    let mut events = request.events.into_iter().peekable();
    let mut held = [None; 2];
    let mut next_report = Instant::now();
    let frames = (|| {
        for index in 0..count {
            pump();
            if cancel.load(Ordering::SeqCst) || unsafe { IsWindow(parent) == 0 } {
                return Err("Export cancelled.".to_owned());
            }
            let pts = index as f64 / f64::from(FPS);
            while let Some(e) = events.next_if(|e| e.time <= pts) {
                let slide = e.slide as u32;
                let previous = &mut held[slide as usize];
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
                            native_mode(e.gesture),
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
            target.frame(initial_phase + pts, pts, |bytes| {
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
