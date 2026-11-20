use super::*;

const WIDTH: usize = 160;
const HEIGHT: usize = 90;

#[derive(Clone, Copy)]
enum Phase {
    Clear,
    Filled,
    Ribbon,
}

fn geometry(phase: Phase) -> Fields {
    const W: usize = 512;
    const H: usize = 288;
    let mut bytes = b"TSSDF001".to_vec();
    for n in [W as u32, H as u32, 2, 24] {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    for _frame in 0..2 {
        for y in 0..H {
            for x in 0..W {
                let d = match phase {
                    Phase::Clear => -48.0,
                    Phase::Filled => 48.0,
                    Phase::Ribbon => {
                        // Isolated vertical capsule, 3.2 source texels across.
                        let dx = x as f32 + 0.5 - 256.0;
                        let dy = ((y as f32 + 0.5 - 144.0).abs() - 45.0).max(0.0);
                        2.5 * (1.6 - dx.hypot(dy))
                    }
                };
                bytes
                    .extend_from_slice(&[((d / 96.0 + 0.5).clamp(0., 1.) * 255.).round() as u8, 0]);
            }
        }
    }
    Fields::from_slice(&bytes).unwrap()
}

fn capture(renderer: &mut LiveWheel, phase: Phase, focus: f32, filtered: bool) -> Vec<u8> {
    renderer.fields = geometry(phase);
    renderer.slots = [None, None];
    renderer.set_focus(Some(focus)).unwrap();
    renderer.set_footprint_sampling(filtered);
    renderer.render_offscreen(0.).unwrap();
    renderer.capture_rgba8().unwrap().2
}

fn linear(value: u8) -> f64 {
    let x = f64::from(value) / 255.;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn absorption(image: &[u8], clear: &[u8], angle: f64) -> (f64, f64, f64) {
    let (s, c) = angle.sin_cos();
    let (mut total, mut moment, mut peak) = (0., 0., 0.0f64);
    for y in 12..HEIGHT - 12 {
        for x in 20..WIDTH - 20 {
            let i = (y * WIDTH + x) * 4 + 1;
            let atten = (1. - linear(image[i]) / linear(clear[i])).max(0.);
            let normal = (x as f64 + 0.5 - WIDTH as f64 * 0.5) * c
                + (y as f64 + 0.5 - HEIGHT as f64 * 0.5) * s;
            total += atten;
            moment += atten * normal * normal;
            peak = peak.max(atten);
        }
    }
    (total, (moment / total).sqrt(), peak)
}

#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn projected_ribbon_retains_tint_through_focus() {
    assert!(std::env::var_os("VK_DRIVER_FILES").is_some());
    let instance = gpu::Instance::new(gpu::InstanceDescriptor {
        backends: gpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapters = instance.enumerate_adapters(gpu::Backends::VULKAN);
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
    let size = [WIDTH as u32, HEIGHT as u32];
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
    let mut r = pollster::block_on(LiveWheel::from_device(
        device,
        queue,
        cfg,
        geometry(Phase::Clear),
        adapter.get_info().name,
        size,
    ))
    .unwrap();
    let angle = 0.41f64;
    r.tracks.entry(r.active).or_default().slides[0].angle = angle;
    r.materials
        .insert("unit-thickness".into(), Material::constant_film());
    r.set_wheel("unit-thickness").unwrap();

    let clear = capture(&mut r, Phase::Clear, 0., true);
    assert_eq!(clear, capture(&mut r, Phase::Clear, 1., true));
    let full = capture(&mut r, Phase::Filled, 0., true);
    let full_far = capture(&mut r, Phase::Filled, 1., true);
    for y in 28..62 {
        for x in 45..115 {
            let i = (y * WIDTH + x) * 4;
            assert_eq!(
                &full[i..i + 4],
                &full_far[i..i + 4],
                "uniform film varied with focus"
            );
            assert!(
                full[i + 1] < clear[i + 1],
                "uniform rear film lost its tint"
            );
        }
    }
    let sharp = capture(&mut r, Phase::Ribbon, 0., true);
    let soft = capture(&mut r, Phase::Ribbon, 1., true);
    let legacy = capture(&mut r, Phase::Ribbon, 0., false);
    if let Some(directory) = std::env::var_os("TSS_FOOTPRINT_PROOF_DIR") {
        use std::{fs::File, io::Write, path::Path};
        for (name, pixels) in [("focused", &sharp), ("defocused", &soft), ("clear", &clear)] {
            let mut file =
                File::create_new(Path::new(&directory).join(format!("{name}.ppm"))).unwrap();
            write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n").unwrap();
            let rgb: Vec<u8> = pixels
                .chunks_exact(4)
                .flat_map(|p| p[..3].iter().copied())
                .collect();
            file.write_all(&rgb).unwrap();
        }
    }
    assert!(
        legacy != sharp,
        "fixture must exercise finite pixel integration"
    );
    let a = absorption(&sharp, &clear, angle);
    let b = absorption(&soft, &clear, angle);
    println!("green channel absorption (sum, RMS width in px, peak): focused {a:?}, defocused {b:?}, reference {:?}",absorption(&legacy,&clear,angle));
    assert!(a.0 > 1. && b.0 > 1.);
    assert!(
        b.1 > a.1 && b.2 < a.2,
        "defocus should broaden and weaken the ribbon"
    );
    assert!(
        (b.0 / a.0 - 1.).abs() < 0.01,
        "excess loss of integrated rear-film attenuation"
    );
}
