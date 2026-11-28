//! Offline, two-eye readback of one evolving stack. Set TSS_LIVE_CAPTURE to a fresh directory.
use super::*;
use crate::single_film_capture::wait_tick;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const SIZE: [u32; 2] = [960, 540];
// Signed half eye separation times focal length, in gate-width-normalized
// pinhole units × mm. A stereo staging choice, independent of iris and focus.
const EYE: f32 = stereo::EYE_SCALE;
const LAYERS: [(u64, f32, u32, u32); 3] = [
    (1, 0.0, 83724, 0x7234cf),
    (2, 0.5, 92843, 0x18d7df),
    (3, 1.0, 17312, 0xff728c),
];

fn frame(r: &mut LiveWheel, eye: f32, t: f64) -> Result<Vec<u8>, String> {
    r.stereo_eye_scale = eye;
    r.upload_material();
    r.render_offscreen(t)?;
    let (w, h, image) = r.capture_rgba8()?;
    assert_eq!([w, h], SIZE);
    Ok(image)
}

// Find a visible oil boundary near the middle to press, using the isolated
// front plane. Prefer vertical edges so horizontal disparity is observable.
fn contact(image: &[u8]) -> [f32; 2] {
    let (w, h) = (SIZE[0] as usize, SIZE[1] as usize);
    let mut best = (0usize, w / 2, h / 2);
    for y in (h / 3..2 * h / 3).step_by(3) {
        for x in (w / 3..2 * w / 3).step_by(3) {
            let i = (y * w + x) * 4;
            let score = (0..3)
                .map(|c| image[i - 12 + c].abs_diff(image[i + 12 + c]) as usize)
                .sum();
            if score > best.0 {
                best = (score, x, y);
            }
        }
    }
    assert!(best.0 > 12, "no oil boundary in front of pressure contact");
    [
        (best.1 as f32 + 0.5) / w as f32,
        (best.2 as f32 + 0.5) / h as f32,
    ]
}

// Return x(right) − x(left) in the original square-pixel eye at a common
// feature. Use RGB differences in a centered symmetrical search window.
fn disparity(left: &[u8], right: &[u8], center: [f32; 2], radius: [i32; 2]) -> i32 {
    let w = SIZE[0] as i32;
    let x0 = (center[0] * w as f32) as i32;
    let y0 = (center[1] * SIZE[1] as f32) as i32;
    let score = |shift: i32| {
        let mut error = 0u64;
        for y in (y0 - radius[1]..y0 + radius[1]).step_by(2) {
            for x in (x0 - radius[0]..x0 + radius[0]).step_by(2) {
                let i = ((w * y + x) * 4) as usize;
                let j = ((w * y + x + shift) * 4) as usize;
                for c in 0..3 {
                    error += u64::from(left[i + c].abs_diff(right[j + c])).pow(2);
                }
            }
        }
        error
    };
    (-14..=14).min_by_key(|dx| score(*dx)).unwrap()
}

fn expected(depth: f32, gap: f64) -> f64 {
    let lens = lens::GateLens::for_projector(false);
    let z = f64::from(lens.distance(depth)) + 0.5 * (1. - gap);
    -2. * f64::from(EYE) * f64::from(SIZE[0]) * (1. / z - 1. / f64::from(lens.distance(0.5)))
}

fn pressure(n: u32) -> f32 {
    let s = ((n as f32 - 10.) / 26.).clamp(0., 1.) * ((96. - n as f32) / 26.).clamp(0., 1.);
    s * s * (3. - 2. * s)
}

#[test]
#[ignore = "requires the reserved discrete GPU, native Basilisk helper, FFmpeg and a fresh output path"]
fn render_stereo_pressure_performance() -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(std::env::var_os("TSS_LIVE_CAPTURE").ok_or("Set TSS_LIVE_CAPTURE")?);
    fs::create_dir(&out)?;
    let worker = PathBuf::from(std::env::var_os("TSS_LIVE_SOLVER").ok_or("Set TSS_LIVE_SOLVER")?);
    let (mut r, adapter) = crate::coral_capture::discrete_renderer(SIZE)?;
    r.set_preview_width(SIZE[0])?;
    r.set_footprint_sampling(true);
    r.set_focus(Some(0.72))?;
    let mut layers = Vec::new();
    for (id, depth, seed, dye) in LAYERS {
        r.set_film_depth(id, Some(depth))?;
        r.set_single_cell_solver(id, &worker, seed, 0.)?;
        wait_tick(&r, id, 0)?;
        layers.push(WheelLayer {
            id,
            wheel: "orchid".into(),
            swatches: Some([0xffffff, dye, dye, 0xffffff]),
            opacity: 1.,
            envelope: None,
            motion: MotionPreset::WheelFlow,
            material_motion: Default::default(),
            films: [true, false],
        });
    }

    r.select_track(3);
    r.set_stack(vec![layers[2].clone()])?;
    let center = contact(&frame(&mut r, 0., 0.)?);
    let mut log = File::create_new(out.join("disparity.csv"))?;
    writeln!(
        log,
        "film,depth,pressure,gap_at_contact,right_minus_left_px,predicted_px"
    )?;
    let mut rear = None;
    for amount in [0., 1.] {
        r.select_track(3);
        r.set_slide_pressure(0, amount, center)?;
        for (i, layer) in layers.iter().enumerate() {
            let depth = LAYERS[i].1;
            r.set_stack(vec![layer.clone()])?;
            let left = frame(&mut r, EYE, 0.)?;
            let right = frame(&mut r, -EYE, 0.)?;
            let (point, window) = if i == 2 {
                (center, [23, 17])
            } else {
                ([0.5, 0.5], [280, 155])
            };
            let measured = disparity(&left, &right, point, window);
            let gap = if i == 2 {
                r.tracks[&3].slides[0]
                    .pressure
                    .inverse(center.map(f64::from))
                    .1
            } else {
                1.
            };
            let predicted = expected(depth, gap);
            writeln!(
                log,
                "{},{depth},{amount},{gap:.4},{measured},{predicted:.3}",
                layer.id
            )?;
            assert!(
                (f64::from(measured) - predicted).abs() < 1.4,
                "film {i} press {amount} measured {measured}, predicted {predicted}"
            );
            if i == 1 {
                assert_eq!(left, right, "convergence layer has residual stereo shift");
            }
            if i == 0 && amount == 0. {
                rear = Some(left)
            } else if i == 0 {
                assert!(rear.as_ref() == Some(&left), "press changed rear film");
            }
        }
    }
    r.select_track(3);
    r.set_slide_pressure(0, 0., center)?;
    r.set_stack(layers)?;
    for (id, _, seed, _) in LAYERS {
        r.set_single_cell_solver(id, &worker, seed, 1.)?;
    }

    let video = out.join("oil-pressure-half-sbs.mp4");
    let mut encode=Command::new("ffmpeg").args(["-v","error","-n","-f","rawvideo","-pixel_format","rgba",
        "-video_size","1920x540","-framerate","24","-i","pipe:0","-an","-vf",
        "scale=1920:1080:out_color_matrix=bt709:out_range=tv,format=yuv444p,colorspace=ispace=bt709:iprimaries=bt709:itrc=srgb:irange=tv:all=bt709:format=yuv420p,setsar=1",
        "-c:v","libx264","-threads","2","-preset","veryfast","-crf","17","-x264-params","frame-packing=3",
        "-movflags","+faststart"]).arg(&video).stdin(Stdio::piped()).spawn()?;
    let mut input = BufWriter::new(encode.stdin.take().unwrap());
    let mut times = File::create_new(out.join("performance.csv"))?;
    writeln!(
        times,
        "frame,seconds,front_pressure,front_contact_gap,rear_tick,middle_tick,front_tick"
    )?;
    let row = SIZE[0] as usize * 4;
    let mut initial = None;
    for n in 0..120u32 {
        let t = f64::from(n) / 24.;
        let p = pressure(n);
        r.set_slide_pressure(0, p, center)?;
        for (id, _, _, _) in LAYERS {
            wait_tick(&r, id, u64::from(n))?;
        }
        let left = frame(&mut r, EYE, t)?;
        let ticks = LAYERS.map(|(id, _, _, _)| r.evolving[&id].displayed_tick());
        let right = frame(&mut r, -EYE, t)?;
        for (i, (id, _, _, _)) in LAYERS.iter().enumerate() {
            assert!((ticks[i] - f64::from(n)).abs() < 1e-6);
            assert_eq!(
                ticks[i],
                r.evolving[id].displayed_tick(),
                "eye sampled a different state"
            );
        }
        let gap = r.tracks[&3].slides[0]
            .pressure
            .inverse(center.map(f64::from))
            .1;
        writeln!(
            times,
            "{n},{t:.6},{p:.6},{gap:.6},{:.4},{:.4},{:.4}",
            ticks[0], ticks[1], ticks[2]
        )?;
        if n == 0 {
            initial = Some(left.clone());
        }
        if n == 119 {
            let count = initial
                .as_ref()
                .unwrap()
                .chunks_exact(4)
                .zip(left.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                count > 1000,
                "live oil failed to evolve at neutral pressure"
            );
        }
        for (a, b) in left.chunks_exact(row).zip(right.chunks_exact(row)) {
            input.write_all(a)?;
            input.write_all(b)?;
        }
    }
    input.flush()?;
    drop(input);
    assert!(encode.wait()?.success(), "stereo movie encoding failed");
    println!(
        "{} {:?}; contact {center:?}; 120 eye pairs of {}x{}",
        adapter.name, adapter.backend, SIZE[0], SIZE[1]
    );
    Ok(())
}
