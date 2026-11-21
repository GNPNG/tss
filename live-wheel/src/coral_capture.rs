use super::*;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    time::Instant,
};

#[path = "coral_solo.rs"]
mod solo;

#[path = "coral_capture/oil_comparison.rs"]
mod oil_comparison;

pub(super) fn discrete_renderer(size: [u32; 2]) -> Result<(LiveWheel, gpu::AdapterInfo), String> {
    let adapter_match =
        std::env::var("TSS_LIVE_ADAPTER").map_err(|_| "Missing hardware adapter name")?;
    if adapter_match.trim().is_empty() {
        return Err("Empty hardware adapter name".into());
    }
    let backend = if cfg!(target_os = "macos") {
        gpu::Backends::METAL
    } else {
        gpu::Backends::VULKAN
    };
    let instance = gpu::Instance::new(gpu::InstanceDescriptor {
        backends: backend,
        ..Default::default()
    });
    let adapter = instance
        .enumerate_adapters(backend)
        .into_iter()
        .find(|candidate| {
            let info = candidate.get_info();
            info.device_type == gpu::DeviceType::DiscreteGpu
                && info
                    .name
                    .to_lowercase()
                    .contains(&adapter_match.to_lowercase())
        })
        .ok_or_else(|| format!("No matching discrete GPU for {adapter_match}"))?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(
        adapter.request_device(
            &gpu::DeviceDescriptor {
                label: Some("Coral release review"),
                required_features: gpu::Features::empty(),
                required_limits: gpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits())
                    .using_alignment(adapter.limits()),
            },
            None,
        ),
    )
    .map_err(|e| e.to_string())?;
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
    let fields = Fields::from_slice(&bundled_fields()?)?;
    let renderer = pollster::block_on(LiveWheel::from_device(
        device,
        queue,
        cfg,
        fields,
        info.name.clone(),
        size,
    ))?;
    Ok((renderer, info))
}

/// Capture the actual editor optics on one explicitly selected hardware GPU.
/// Each readback finishes before the next optical frame is submitted.
#[test]
#[ignore = "requires a reserved hardware GPU and an unused capture directory"]
fn capture_coral_release() -> Result<(), String> {
    let path =
        PathBuf::from(std::env::var_os("TSS_CORAL_CAPTURE_DIR").ok_or("Missing capture path")?);
    let size = [640, 360];
    let (mut renderer, info) = discrete_renderer(size)?;
    renderer.set_wheel("violet-blue")?;
    renderer.set_focus(Some(1.0))?;
    let capture = |r: &mut LiveWheel| -> Result<Vec<u8>, String> {
        r.render_offscreen(0.0)?;
        Ok(r.capture_rgba8()?.2)
    };
    let original = capture(&mut renderer)?;
    renderer.set_motion(MotionPreset::CoralRelease)?;
    let release = capture(&mut renderer)?;
    let changed = original
        .chunks_exact(4)
        .zip(release.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) >= 3))
        .count();
    assert!(
        changed > 0,
        "Switching motion must change the projected geometry"
    );
    renderer.set_motion(MotionPreset::WheelFlow)?;
    assert!(
        capture(&mut renderer)? == original,
        "Restoring wheel flow must restore its source geometry"
    );
    renderer.set_motion(MotionPreset::CoralRelease)?;
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    if std::env::var_os("TSS_CORAL_TUG_COMPARISON").is_some() {
        return tug_comparison(&mut renderer, &path);
    }
    let mut sink =
        BufWriter::new(File::create_new(path.join("frames.rgba")).map_err(|e| e.to_string())?);
    let mut timings_ms = Vec::new();
    let rate = 24;
    let frames = 7 * rate;
    for index in 0..frames {
        let t = f64::from(index) / f64::from(rate);
        // Watch the complete release at front focus. Hold the final state to examine
        // the same droplets and the rear plume while focus travels away and back.
        let ramp = |s: f64| {
            let u = s.clamp(0.0, 1.0);
            u * u * (3.0 - 2.0 * u)
        };
        let focus = 1.0 - ramp((t - 4.0) / 0.85) + ramp((t - 5.5) / 1.0);
        renderer.set_focus(Some(focus as f32))?;
        let before = Instant::now();
        renderer.render_program_offscreen(t, t)?;
        let (w, h, pixels) = renderer.capture_rgba8()?;
        timings_ms.push(before.elapsed().as_secs_f64() * 1000.0);
        assert_eq!([w, h], size);
        sink.write_all(&pixels).map_err(|e| e.to_string())?;
    }
    sink.flush().map_err(|e| e.to_string())?;
    let metadata = serde_json::json!({
        "gpu": info.name, "backend": format!("{:?}", info.backend),
        "wheel": "violet-blue", "motion": "coral-release", "size": size,
        "frames": frames, "fps": rate, "format": "rgba8-srgb",
        "focus": "front 0–4 s; rear 4.85–5.5 s; front again at 6.5 s",
        "motion_switch_changed_pixels_at_least_3": changed, "restored_wheel_flow_exactly": true,
        "render_plus_readback_ms": timings_ms,
    });
    fs::write(
        path.join("capture.json"),
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn tug_comparison(renderer: &mut LiveWheel, path: &std::path::Path) -> Result<(), String> {
    // The unmatched instruction is one front-slide drag. Both tracks receive
    // the same simulation sample, camera focus and shader configuration.
    let mut left =
        BufWriter::new(File::create_new(path.join("no-tug.rgba")).map_err(|e| e.to_string())?);
    let mut right =
        BufWriter::new(File::create_new(path.join("front-tug.rgba")).map_err(|e| e.to_string())?);
    let point = |frame: usize| -> [f32; 2] {
        let u = ((frame as f32 - 36.) / 24.).clamp(0., 1.);
        let p = u * u * (3. - 2. * u);
        [0.46 + 0.13 * p, 0.58 - 0.13 * p]
    };
    let mut counts = Vec::new();
    for n in 0..132usize {
        let seconds = n as f64 / 24.;
        renderer.select_track(1);
        renderer.render_offscreen(seconds)?;
        let (_, _, a) = renderer.capture_rgba8()?;
        renderer.select_track(2);
        if n == 36 {
            renderer.set_tug_response(1, 1.)?;
        }
        if (37..=60).contains(&n) {
            renderer.gesture(1, SlideGesture::Tug, point(n - 1), point(n));
        }
        if n == 61 {
            renderer.end_gesture(1);
        }
        renderer.render_offscreen(seconds)?;
        let (_, _, b) = renderer.capture_rgba8()?;
        if n <= 36 {
            assert!(
                a == b,
                "Matched controls differ before contact at frame {n}"
            );
        }
        counts.push(
            a.chunks_exact(4)
                .zip(b.chunks_exact(4))
                .filter(|(aa, bb)| (0..3).any(|k| aa[k].abs_diff(bb[k]) >= 3))
                .count(),
        );
        left.write_all(&a).map_err(|e| e.to_string())?;
        right.write_all(&b).map_err(|e| e.to_string())?;
    }
    assert!(counts[60] > 0 && counts[131] > 0);
    left.flush().map_err(|e| e.to_string())?;
    right.flush().map_err(|e| e.to_string())?;
    let meta = serde_json::json!({
        "adapter":renderer.adapter_name(), "focus":"front", "wheel":"violet-blue",
        "fps":24, "frames":132, "size":[640,360], "motion":"coral-release",
        "tug":{"slide":"front", "response":1.0,"start_seconds":1.5,"end_seconds":2.5,
               "from":point(36),"to":point(60),"path":"smoothstep"},
        "pre_contact_identical":true, "changed_pixels_at_least_three_codes":counts
    });
    fs::write(
        path.join("comparison.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
#[ignore = "requires a reserved discrete GPU and a new TSS_CORAL_CAPTURE_DIR"]
fn capture_coral_wheel() -> Result<(), String> {
    let path =
        PathBuf::from(std::env::var_os("TSS_CORAL_CAPTURE_DIR").ok_or("Missing capture path")?);
    let size = [960, 540];
    let (mut renderer, info) = discrete_renderer(size)?;
    renderer.set_preview_width(size[0])?;
    renderer.set_footprint_sampling(true);
    renderer.set_wheel("violet-blue")?;
    renderer.set_motion(MotionPreset::WheelWithCoralBeads)?;
    renderer.set_focus(Some(0.90))?;
    let layer_proof = crate::coral_layer_proof::verify(&mut renderer)?;
    let sample = |r: &mut LiveWheel, rear: f64, front: f64| -> Result<Vec<u8>, String> {
        r.render_film_times_offscreen(rear, front, rear)?;
        let (w, h, rgba) = r.capture_rgba8()?;
        if [w, h] != size {
            return Err(format!("Unexpected capture extent {w} × {h}"));
        }
        Ok(rgba)
    };
    // With both source times pinned, only the clock under test may change pixels.
    let changed = |a: &[u8], b: &[u8]| {
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(p, q)| (0..3).any(|k| p[k].abs_diff(q[k]) >= 3))
            .count()
    };
    let first = sample(&mut renderer, 0., 0.)?;
    let rear_only = sample(&mut renderer, 1.5, 0.)?;
    let front_only = sample(&mut renderer, 0., 3.5)?;
    let rear_late = sample(&mut renderer, 5.5, 7.)?;
    let end = sample(&mut renderer, 4., 4.)?;
    let held_front = sample(&mut renderer, 4., 7.)?;
    assert!(changed(&first, &rear_only) > 0 && changed(&first, &front_only) > 0);
    assert!(held_front == end && changed(&end, &rear_late) > 0);
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    let mut sink =
        BufWriter::new(File::create_new(path.join("frames.rgba")).map_err(|e| e.to_string())?);
    let mut timings = Vec::new();
    for n in 0..144 {
        let t = f64::from(n) / 24.;
        let start = Instant::now();
        let frame = sample(&mut renderer, t, t)?;
        timings.push(start.elapsed().as_secs_f64() * 1000.);
        sink.write_all(&frame).map_err(|e| e.to_string())?;
    }
    sink.flush().map_err(|e| e.to_string())?;
    let capture = serde_json::json!({
        "adapter":info.name,"backend":format!("{:?}",info.backend),"size":size,
        "motion":"wheel-with-coral-beads","wheel":"violet-blue","fps":24,"frames":144,"focus":0.90,
        "format":"full-range sRGB RGBA8", "finite_pixel_sampling": true, "layer_proof": layer_proof,
        "source_clock_checks":{
            "changed_pixels_at_least_three_codes":{
                "rear_0_to_1p5_front_zero":changed(&first,&rear_only),
                "front_0_to_3p5_rear_zero":changed(&first,&front_only),
                "rear_4_to_5p5_front_held":changed(&end,&rear_late)},
            "front_at_4_and_7_identical_when_rear_held_at_4":true},
        "render_plus_readback_ms":timings
    });
    fs::write(
        path.join("capture.json"),
        serde_json::to_vec_pretty(&capture).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
#[ignore = "requires one guarded discrete GPU and an existing TSS_PLATE_CAPTURE_DIR"]
fn capture_digital_4k_plates() -> Result<(), String> {
    let dir = PathBuf::from(
        std::env::var_os("TSS_PLATE_CAPTURE_DIR").ok_or("Missing plate output folder")?,
    );
    let (mut r, _) = discrete_renderer([320, 180])?;
    r.select_track(1);
    r.set_motion(MotionPreset::WheelWithCoralBeads)?;
    r.set_focus(Some(0.9))?;
    r.set_footprint_sampling(true);
    let times = [2.635, 0.75];
    r.render_film_times_offscreen(times[0], times[1], times[0])?;
    let preview = r.capture_rgba8()?.2;
    crate::plates::check_preview_alignment(&mut r, times, &preview)?;
    let bundle = pollster::block_on(r.export_4k_plates(&dir, times, |_| Ok(())))?;
    r.render_film_times_offscreen(times[0], times[1], times[0])?;
    assert_eq!(
        r.capture_rgba8()?.2,
        preview,
        "Plate export changed the live pose or optics"
    );
    println!("Plate bundle {}", bundle.display());
    Ok(())
}

#[test]
#[ignore = "requires a guarded discrete GPU and an existing TSS_PLATE_CAPTURE_DIR"]
fn capture_production_formats() -> Result<(), String> {
    use crate::production::{DigitalFormat, Encoding, Layout};
    let dir = PathBuf::from(
        std::env::var_os("TSS_PLATE_CAPTURE_DIR").ok_or("Missing plate output folder")?,
    );
    let (mut r, _) = discrete_renderer([320, 180])?;
    r.select_track(1);
    r.set_motion(MotionPreset::WheelWithCoralBeads)?;
    r.set_focus(Some(0.9))?;
    r.set_footprint_sampling(true);
    let times = [2.635, 0.75];
    r.render_film_times_offscreen(times[0], times[1], times[0])?;
    let preview = r.capture_rgba8()?.2;
    crate::plates::check_preview_alignment(&mut r, times, &preview)?;
    for (layout, encoding) in [
        (Layout::DspCover, Encoding::Srgb),
        (Layout::Vertical, Encoding::Rec709),
        (Layout::Panorama, Encoding::DisplayP3),
        (Layout::Visualizer, Encoding::DisplayP3),
    ] {
        let format = DigitalFormat {
            layout,
            encoding,
            horizon_y: (layout == Layout::Visualizer).then_some(1339),
        };
        let bundle = pollster::block_on(r.export_digital_plates(&dir, times, format, |_| Ok(())))?;
        println!("{layout:?} {}", bundle.display());
    }
    Ok(())
}

#[test]
#[ignore = "requires a guarded discrete GPU, TSS_PLATE_CAPTURE_DIR and TSS_ECI_PROFILE_DIR"]
fn capture_print_mechanicals() -> Result<(), String> {
    use crate::production::{PrintFormat, PrintLayout, Stock};
    let dir =
        PathBuf::from(std::env::var_os("TSS_PLATE_CAPTURE_DIR").ok_or("Missing output directory")?);
    let iccs =
        PathBuf::from(std::env::var_os("TSS_ECI_PROFILE_DIR").ok_or("Missing ECI profiles")?);
    let (mut r, info) = discrete_renderer([320, 180])?;
    r.select_track(1);
    r.set_motion(MotionPreset::WheelWithCoralBeads)?;
    r.set_focus(Some(0.9))?;
    r.set_footprint_sampling(true);
    let times = [2.635, 0.75];
    r.render_film_times_offscreen(times[0], times[1], times[0])?;
    println!("Native optical source: {}", info.name);
    for (layout, stock) in [
        (PrintLayout::LpGatefold, Stock::Coated),
        (PrintLayout::Label7, Stock::Uncoated),
    ] {
        let bytes = std::fs::read(iccs.join(stock.profile_name())).map_err(|e| e.to_string())?;
        let mut profile = crate::PrintProfile::new(stock, bytes)?;
        let format = PrintFormat {
            layout,
            stock,
            catalog: Some(6),
            gilding: Some(if stock == Stock::Coated {
                crate::production::Gilding::Metallic
            } else {
                crate::production::Gilding::Foil
            }),
            rich_black: true,
        };
        let [w, h] = layout.mechanical().pixels;
        // Known independent registration marks exercise exact process and spot output.
        let mask = |right: bool| -> Result<Vec<u8>, String> {
            let stride = w.div_ceil(8) as usize;
            let mut bits = vec![255; stride * h as usize];
            let x0 = if right { w * 3 / 5 } else { w / 5 };
            for y in h / 3..h / 3 + 24 {
                for x in x0..x0 + 32 {
                    bits[y as usize * stride + x as usize / 8] &= !(0x80 >> (x % 8));
                }
            }
            let mut data = Vec::new();
            let mut e = png::Encoder::new(&mut data, w, h);
            e.set_color(png::ColorType::Grayscale);
            e.set_depth(png::BitDepth::One);
            let mut writer = e.write_header().map_err(|e| e.to_string())?;
            writer.write_image_data(&bits).map_err(|e| e.to_string())?;
            writer.finish().map_err(|e| e.to_string())?;
            Ok(data)
        };
        profile.set_masks(format, Some(mask(true)?), Some(mask(false)?))?;

        let dir =
            pollster::block_on(
                r.export_print_mechanical(&dir, times, format, &profile, |_| Ok(())),
            )?;
        println!("{layout:?}: {}", dir.display());
    }
    Ok(())
}
