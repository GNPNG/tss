use super::*;

pub(super) fn fixture(size: [u32; 2]) -> LiveWheel {
    assert!(std::env::var_os("VK_DRIVER_FILES").is_some());
    let vulkan = gpu::Instance::new(gpu::InstanceDescriptor {
        backends: gpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapters = vulkan.enumerate_adapters(gpu::Backends::VULKAN);
    assert!(
        !adapters.is_empty()
            && adapters
                .iter()
                .all(|a| a.get_info().device_type == gpu::DeviceType::Cpu)
    );
    let adapter = &adapters[0];
    let (device, queue) =
        pollster::block_on(adapter.request_device(&gpu::DeviceDescriptor::default(), None))
            .unwrap();
    let config = gpu::SurfaceConfiguration {
        usage: gpu::TextureUsages::RENDER_ATTACHMENT,
        format: gpu::TextureFormat::Bgra8Unorm,
        width: size[0],
        height: size[1],
        present_mode: gpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: gpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
    };
    pollster::block_on(LiveWheel::from_device(
        device,
        queue,
        config,
        Fields::from_slice(&bundled_fields().unwrap()).unwrap(),
        adapter.get_info().name,
        size,
    ))
    .unwrap()
}

fn linear(v: u8) -> f64 {
    let x = f64::from(v) / 255.;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}
fn frame(r: &mut LiveWheel, wheel: &str, time: f64, pts: f64) -> Vec<u8> {
    r.set_wheel(wheel).unwrap();
    r.render_program_to(None, time, pts);
    r.capture_rgba8().unwrap().2
}

#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn timestamped_feedback_reads_completed_program() {
    let mut r = fixture([128, 72]);
    let phases = [
        ("violet-blue", 0., 0.),
        ("violet-blue", 1., 0.043),
        ("blue-red", 2., 0.105),
        ("blue-green", 3., 0.153),
        ("yellow-red", 1.6, 0.208),
    ];
    let base: Vec<_> = phases
        .iter()
        .map(|&(w, t, p)| frame(&mut r, w, t, p))
        .collect();
    assert_ne!(base[0], base[1], "same wheel must move");
    r.set_feedback(Some(FeedbackSettings {
        delay_ms: 100,
        strength: 0.6,
        zoom: 1.,
        rotation_degrees: 0.,
    }))
    .unwrap();
    let actual: Vec<_> = phases
        .iter()
        .map(|&(w, t, p)| frame(&mut r, w, t, p))
        .collect();
    let mut count = 0;
    let (mut recursive_error, mut feedforward_error) = (0., 0.);
    for k in 0..base[0].len() {
        if k % 4 == 3 || (0..5).any(|i| base[i][k] > 245 || actual[i][k] > 245) {
            continue;
        }
        count += 1;
        // .208 s reads the completed .105 s frame; .105 s itself reads time zero.
        let base_at_105 = linear(base[2][k]);
        let expected_at_105 = 0.4 * base_at_105 + 0.6 * linear(base[0][k]);
        let e = 0.4 * linear(base[4][k]) + 0.6 * expected_at_105;
        let one_tap = 0.4 * linear(base[4][k]) + 0.6 * base_at_105;
        let got = linear(actual[4][k]);
        recursive_error += (got - e).abs();
        feedforward_error += (got - one_tap).abs();
    }
    let mean = recursive_error / f64::from(count);
    let wrong = feedforward_error / f64::from(count);
    println!("{count} unclipped channel values: recursive mean absolute error {mean:.6}, delayed-base error {wrong:.6}");
    assert!(count > 10000 && mean < 0.004 && wrong > mean * 4.);
    let reset = frame(&mut r, phases[0].0, phases[0].1, 0.001);
    assert!(
        reset.iter().zip(&base[0]).all(|(a, b)| a.abs_diff(*b) <= 1),
        "rewind retained stale feedback"
    );
    r.set_feedback(None).unwrap();
    assert_eq!(frame(&mut r, phases[0].0, phases[0].1, 0.001), base[0]);
}

#[test]
#[ignore = "writes a review clip sequence using an isolated software Vulkan ICD"]
fn record_feedback_comparison() {
    use std::{fs::File, io::Write, path::PathBuf};
    let dir = PathBuf::from(
        std::env::var_os("TSS_FEEDBACK_CLIP_DIR").expect("set a fresh output directory"),
    );
    let [w, h] = [320, 180];
    let mut r = fixture([w, h]);
    for enabled in [false, true] {
        r.set_feedback(enabled.then_some(FeedbackSettings {
            delay_ms: 125,
            strength: 0.78,
            zoom: 1.06,
            rotation_degrees: 3.,
        }))
        .unwrap();
        for i in 0..72 {
            let t = i as f64 / 24.;
            let color = if i < 24 {
                "violet-blue"
            } else if i < 48 {
                "blue-red"
            } else {
                "blue-green"
            };
            let pixels = frame(&mut r, color, t, t);
            let label = if enabled { "feedback" } else { "source" };
            let mut file = File::create_new(dir.join(format!("{label}-{i:03}.ppm"))).unwrap();
            write!(file, "P6\n{w} {h}\n255\n").unwrap();
            let rgb: Vec<_> = pixels
                .chunks_exact(4)
                .flat_map(|x| x[..3].iter().copied())
                .collect();
            file.write_all(&rgb).unwrap();
        }
    }
}

#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn finite_optics_stays_continuous_at_swirl_onset() {
    let mut r = fixture([320, 180]);
    r.set_footprint_sampling(true);
    r.set_focus(Some(0.5)).unwrap();
    let still = frame(&mut r, "violet-blue", 1.25, 0.0);
    r.tracks.entry(r.active).or_default().slides[0].angle = 1e-8;
    let onset = frame(&mut r, "violet-blue", 1.25, 0.025);
    let deltas: Vec<_> = still
        .chunks_exact(4)
        .zip(onset.chunks_exact(4))
        .map(|(a, b)| (0..3).map(|k| a[k].abs_diff(b[k])).max().unwrap())
        .collect();
    let max = *deltas.iter().max().unwrap();
    let count = deltas.iter().filter(|&&d| d >= 3).count();
    println!(
        "swirl onset 1e-8 rad: {count}/{} pixels change >=3 codes; max {max}",
        deltas.len()
    );
    assert!(
        count == 0,
        "finite pupil must remain continuous on first motion"
    );
}
