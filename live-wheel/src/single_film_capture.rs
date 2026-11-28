use super::*;
use std::{
    env, fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const SIZE: [u32; 2] = [640, 360];
const DYES: [u32; 3] = [0xfd3d0c, 0x308bc0, 0xfef052];
const DEPTHS: [f32; 3] = [0., 0.5, 1.];
const SEEDS: [u32; 3] = [83724, 92843, 17312];

fn capture(renderer: &mut LiveWheel, time: f64) -> Result<Vec<u8>, String> {
    renderer.render_offscreen(time)?;
    let (w, h, pixels) = renderer.capture_rgba8()?;
    assert_eq!([w, h], SIZE);
    Ok(pixels)
}

pub(super) fn wait_tick(renderer: &LiveWheel, id: u64, tick: u64) -> Result<(), String> {
    let start = Instant::now();
    loop {
        let status = renderer.live_solver_status(id).ok_or("lost cell")?;
        if let Some(e) = status.error {
            return Err(e);
        }
        if status.state == "ready" && status.tick >= tick {
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(20) {
            return Err(format!("cell {id} reached tick {} of {tick}", status.tick));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn frame_statistics(pixels: &[u8]) -> ([f64; 3], f64) {
    let mut sums = [0.; 3];
    let mut energy = 0.;
    for (n, p) in pixels.chunks_exact(4).enumerate() {
        for c in 0..3 {
            sums[c] += f64::from(p[c]);
            if n % SIZE[0] as usize != 0 {
                energy += (f64::from(p[c]) - f64::from(pixels[(n - 1) * 4 + c])).powi(2);
            }
        }
    }
    (sums.map(|sum| sum / f64::from(SIZE[0] * SIZE[1])), energy)
}

pub(super) fn write_png(path: &Path, pixels: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut enc = png::Encoder::new(
        BufWriter::new(fs::File::create_new(path)?),
        SIZE[0],
        SIZE[1],
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(pixels)?;
    Ok(())
}

#[test]
#[ignore = "requires a reserved discrete GPU, native Basilisk worker and a fresh capture path"]
fn three_single_cells_preserve_depth_and_transmission() -> Result<(), Box<dyn std::error::Error>> {
    let worker = PathBuf::from(env::var_os("TSS_LIVE_SOLVER").ok_or("Set TSS_LIVE_SOLVER")?);
    let out = PathBuf::from(env::var_os("TSS_LIVE_CAPTURE").ok_or("Set TSS_LIVE_CAPTURE")?);
    fs::create_dir(&out)?;
    let (mut r, adapter) = crate::coral_capture::discrete_renderer(SIZE)?;
    r.set_preview_width(SIZE[0])?;
    r.set_footprint_sampling(true);
    let layers = std::array::from_fn::<_, 3, _>(|i| {
        let dye = DYES[i];
        WheelLayer {
            id: i as u64 + 1,
            wheel: "orchid".into(),
            swatches: Some([0xffffff, dye, dye, 0xffffff]),
            opacity: 1.,
            envelope: None,
            motion: MotionPreset::WheelFlow,
            material_motion: Default::default(),
            films: [true, false],
        }
    });
    let mut log = fs::File::create_new(out.join("focus.csv"))?;
    writeln!(
        log,
        "layer,depth,focus,mean_r,mean_g,mean_b,horizontal_edge_energy"
    )?;
    for (i, l) in layers.iter().enumerate() {
        r.set_film_depth(l.id, Some(DEPTHS[i]))?;
        r.set_single_cell_solver(l.id, &worker, SEEDS[i], 0.)?;
        wait_tick(&r, l.id, 0)?;
    }
    let mut clear = layers[0].clone();
    clear.opacity = 0.;
    r.set_stack(vec![clear])?;
    let (open_lamp, _) = frame_statistics(&capture(&mut r, 0.)?);
    for (i, l) in layers.iter().enumerate() {
        r.set_stack(vec![l.clone()])?;
        let mut energy = [0.; 3];
        let mut means = [[0.; 3]; 3];
        for (j, focus) in DEPTHS.into_iter().enumerate() {
            r.set_focus(Some(focus))?;
            let pixels = capture(&mut r, 0.)?;
            (means[j], energy[j]) = frame_statistics(&pixels);
            let [red, green, blue] = means[j];
            writeln!(
                log,
                "{},{},{focus},{red:.4},{green:.4},{blue:.4},{}",
                l.id, DEPTHS[i], energy[j]
            )?;
            if j == i {
                write_png(&out.join(format!("film-{}.png", l.id)), &pixels)?;
            }
        }
        assert!(
            energy
                .iter()
                .enumerate()
                .all(|(j, e)| j == i || energy[i] > *e),
            "edge energies {energy:?} at depth {}",
            DEPTHS[i]
        );
        // Sharpness changes while the projected dye continues filtering light.
        for c in 0..3 {
            assert!(
                means.iter().all(|m| (m[c] - means[i][c]).abs() < 4.),
                "channel means {means:?}"
            );
        }
        // Remove the projector's gate falloff and illuminant tint before comparing dyes.
        let [red, green, blue] = std::array::from_fn(|c| means[i][c] / open_lamp[c]);
        assert!(
            match i {
                0 => red > green && green > blue,
                1 => blue > red && green > red,
                _ => red > blue && green > blue,
            },
            "wrong solo dye {i}: {:?}",
            means[i]
        );
        let before = capture(&mut r, 0.)?;
        let mut with_hidden = layers.to_vec();
        for other in &mut with_hidden {
            if other.id != l.id {
                other.opacity = 0.;
            }
        }
        r.set_stack(with_hidden)?;
        assert!(
            before == capture(&mut r, 0.)?,
            "hidden neighbour changed film {}",
            l.id
        );
    }
    r.set_stack(layers.to_vec())?;
    for (i, l) in layers.iter().enumerate() {
        r.set_single_cell_solver(l.id, &worker, SEEDS[i], 1.)?;
    }
    r.select_track(layers[1].id);
    let mut raw = BufWriter::new(fs::File::create_new(out.join("projection.rgba"))?);
    for frame in 0..96 {
        let t = frame as f64 / 24. + 1e-6;
        r.set_focus(Some((frame as f32 / 80.).min(1.)))?;
        if (25..40).contains(&frame) {
            let x = 0.34 + (frame - 25) as f32 * 0.012;
            r.gesture(0, SlideGesture::Tug, [x, 0.52], [x + 0.012, 0.525]);
        }
        if frame == 40 {
            r.end_gesture(0);
        }
        r.render_offscreen(t)?;
        for l in &layers {
            wait_tick(&r, l.id, frame as u64 + 1)?;
        }
        let pixels = capture(&mut r, t)?;
        if [0, 40, 80].contains(&frame) {
            write_png(&out.join(format!("focus-{frame:02}.png")), &pixels)?;
        }
        raw.write_all(&pixels)?;
        thread::sleep(Duration::from_millis(18));
    }
    raw.flush()?;
    println!(
        "{} {:?}, {} × {}, 96 frames / 24 fps; {}",
        adapter.name,
        adapter.backend,
        SIZE[0],
        SIZE[1],
        out.display()
    );
    Ok(())
}
