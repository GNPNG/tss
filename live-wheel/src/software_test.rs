use super::*;

/// Run explicitly with VK_DRIVER_FILES restricted to a software Vulkan ICD.
/// Executes the production render path through the final blit into a texture.
#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn render_wheels_on_software_vulkan() {
    assert!(std::env::var_os("VK_DRIVER_FILES").is_some());
    let instance = gpu::Instance::new(gpu::InstanceDescriptor {
        backends: gpu::Backends::VULKAN,
        ..Default::default()
    });
    let mut adapters = instance.enumerate_adapters(gpu::Backends::VULKAN);
    assert!(!adapters.is_empty(), "No Vulkan adapter found");
    assert!(adapters
        .iter()
        .all(|a| a.get_info().device_type == gpu::DeviceType::Cpu));
    let adapter = adapters.remove(0);
    let (device, queue) = pollster::block_on(adapter.request_device(
        &gpu::DeviceDescriptor {
            label: Some("Software projection verification"),
            required_features: gpu::Features::empty(),
            required_limits: gpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        },
        None,
    ))
    .unwrap();
    let proof_dir = std::env::var_os("TSS_SOFTWARE_PROOF_DIR").map(std::path::PathBuf::from);
    let replay_path = std::env::var_os("TSS_SOFTWARE_REPLAY_TIMES");
    let size = if proof_dir.is_some() || replay_path.is_some() {
        [640, 360]
    } else {
        [128, 72]
    };
    let cfg = gpu::SurfaceConfiguration {
        usage: gpu::TextureUsages::RENDER_ATTACHMENT,
        format: gpu::TextureFormat::Bgra8Unorm,
        width: size[0],
        height: size[1],
        present_mode: gpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: gpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
    };
    let target = Target::new(&device, "Software viewport", size, cfg.format);
    let raw = if replay_path.is_some() || proof_dir.is_some() {
        fields::legacy_fields().unwrap()
    } else {
        bundled_fields().unwrap()
    };
    let fields = Fields::from_slice(&raw).unwrap();
    let mut renderer = pollster::block_on(LiveWheel::from_device(
        device,
        queue,
        cfg,
        fields,
        adapter.get_info().name,
        size,
    ))
    .unwrap();
    if let Some(times) = replay_path {
        use std::{fs, io::Write};
        renderer.set_wheel("violet-blue").unwrap();
        let out = std::path::PathBuf::from(std::env::var_os("TSS_SOFTWARE_REPLAY_OUTPUT").unwrap());
        for (index, line) in fs::read_to_string(times).unwrap().lines().enumerate() {
            let time: f64 = line.parse().unwrap();
            renderer.render_offscreen(time).unwrap();
            let (w, h, rgba) = renderer.capture_rgba8().unwrap();
            let mut file = fs::File::create_new(out.join(format!("frame-{index:04}.ppm"))).unwrap();
            write!(file, "P6\n{w} {h}\n255\n").unwrap();
            let rgb: Vec<_> = rgba
                .chunks_exact(4)
                .flat_map(|p| p[..3].iter().copied())
                .collect();
            file.write_all(&rgb).unwrap();
        }
        return;
    }
    renderer
        .device
        .push_error_scope(gpu::ErrorFilter::Validation);
    let capture = |r: &mut LiveWheel| {
        r.render_to(Some(&target.view), 1.25);
        r.capture_rgba8().unwrap().2
    };
    let mut frames = std::collections::HashSet::new();
    let ids: Vec<_> = renderer.materials.keys().cloned().collect();
    for id in ids {
        renderer.set_wheel(&id).unwrap();
        assert!(
            frames.insert(capture(&mut renderer)),
            "Duplicate wheel {id}"
        );
    }
    assert_eq!(frames.len(), 7);
    renderer.set_wheel("violet-blue").unwrap();
    renderer.select_track(1);
    let baseline = capture(&mut renderer);
    assert!(renderer.render_offscreen(f64::NAN).is_err());
    renderer.render_to(Some(&target.view), 2.25);
    assert!(
        renderer.capture_rgba8().unwrap().2 != baseline,
        "Different test times produced identical pixels"
    );
    renderer.render_offscreen(1.25).unwrap();
    assert!(
        renderer.capture_rgba8().unwrap().2 == baseline,
        "Offscreen optics differ from the displayed frame"
    );
    renderer.swirl(1, [0.40, 0.45], [0.55, 0.52]);
    let after = capture(&mut renderer);
    let changed = baseline
        .chunks_exact(4)
        .zip(after.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) >= 3))
        .count();
    assert!(changed > 0, "Front slide swirl");
    if proof_dir.is_none() {
        for (current, expected) in [(true, &after), (false, &baseline)] {
            let mut movie = pollster::block_on(renderer.begin_movie(size, current)).unwrap();
            movie
                .frame(1.25, 0., |pixels| {
                    assert_eq!(
                        pixels,
                        expected.as_slice(),
                        "Export baseline differed from the native optical frame"
                    );
                    Ok(())
                })
                .unwrap();
            movie
                .gesture(0, SlideGesture::Rotate, 0.4, [0.75, 0.50], [0.50, 0.80])
                .unwrap();
            let error = movie
                .frame(1.25, 1. / 30., |pixels| {
                    assert_ne!(pixels, expected.as_slice());
                    Err("Intentional encoder failure".into())
                })
                .unwrap_err();
            assert_eq!(error, "Intentional encoder failure");
            drop(movie);
            assert_eq!(
                capture(&mut renderer),
                after,
                "Export mutated the live track"
            );
        }
        if std::env::var_os("TSS_VERIFY_NATIVE_UHD").is_some() {
            let mut movie = pollster::block_on(renderer.begin_movie([3840, 2160], false)).unwrap();
            movie
                .frame(1.25, 0., |pixels| {
                    assert_eq!(pixels.len(), 3840 * 2160 * 4);
                    assert!(pixels.chunks_exact(4).any(|p| p[..3] != pixels[..3]));
                    assert!(pixels.chunks_exact(4).all(|p| p[3] == 255));
                    println!(
                        "Software Vulkan evaluated a full 3840 × 2160 native RGBA8 optical frame"
                    );
                    Ok(())
                })
                .unwrap();
            drop(movie);
            assert_eq!(capture(&mut renderer), after);
        }
    }
    renderer.select_track(2);
    assert!(
        capture(&mut renderer) == baseline,
        "Other track retained a stroke"
    );
    if let Some(dir) = proof_dir {
        use std::io::Write;
        for (label, pixels) in [("before", &baseline), ("after", &after)] {
            let mut out = std::fs::File::create_new(dir.join(format!("{label}.ppm"))).unwrap();
            write!(out, "P6\n{} {}\n255\n", size[0], size[1]).unwrap();
            let rgb: Vec<_> = pixels
                .chunks_exact(4)
                .flat_map(|p| p[..3].iter().copied())
                .collect();
            out.write_all(&rgb).unwrap();
        }
        println!("{} × {} CPU Vulkan: {changed} pixels changed by >=3 RGB codes, untouched track identical", size[0], size[1]);
    }
    renderer.set_focus(Some(0.0)).unwrap();
    let rear = capture(&mut renderer);
    renderer.set_focus(Some(1.0)).unwrap();
    assert!(capture(&mut renderer) != rear, "Focus endpoints match");
    assert!(pollster::block_on(renderer.device.pop_error_scope()).is_none());
}
