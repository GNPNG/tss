use super::*;
use crate::warp_diagnostic::{half, read_rgba16};
use std::{io::Write, path::Path};

fn picture(r: &mut LiveWheel) -> Vec<u8> {
    r.render_offscreen(1.25).unwrap();
    r.capture_rgba8().unwrap().2
}
fn write_ppm(path: &Path, size: [u32; 2], rgba: &[u8]) {
    let mut f = std::io::BufWriter::new(std::fs::File::create_new(path).unwrap());
    write!(f, "P6\n{} {}\n255\n", size[0], size[1]).unwrap();
    for row in rgba.chunks_exact(size[0] as usize * 4) {
        let rgb: Vec<_> = row
            .chunks_exact(4)
            .flat_map(|v| v[..3].iter().copied())
            .collect();
        f.write_all(&rgb).unwrap();
    }
}

#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn colored_pressure_follows_one_slide_and_releases_exactly() {
    let size = [320, 180];
    let mut r = crate::feedback_test::fixture(size);
    r.select_track(7);
    let before = picture(&mut r);
    let field_before = read_rgba16(&r, &r.interpolated, r.field_size());
    assert!(r.set_slide_pressure(2, 1., [0.5; 2]).is_err());
    assert!(r.set_slide_pressure(0, f32::NAN, [0.5; 2]).is_err());
    assert!(r.set_slide_pressure(0, 0.5, [2., 0.5]).is_err());
    for (slide, center) in [[0.365, 0.48], [0.58, 0.41]].into_iter().enumerate() {
        r.set_slide_pressure(slide as u32, 1., center).unwrap();
        let pressed = picture(&mut r);
        let field = read_rgba16(&r, &r.interpolated, r.field_size());
        let mut changed = 0;
        for (new, old) in field.iter().zip(&field_before) {
            assert_eq!(new[1 - slide], old[1 - slide]);
            assert_eq!(new[3 - slide], old[3 - slide]);
            changed += usize::from(new[slide] != old[slide]);
        }
        let rgb_changes = pressed
            .chunks_exact(4)
            .zip(before.chunks_exact(4))
            .filter(|(a, b)| (0..3).any(|k| a[k].abs_diff(b[k]) >= 3))
            .count();
        println!("slide {slide}: {changed} changed phase texels; opposite phase and footprint identical; {rgb_changes} changed RGB pixels");
        assert!(changed > 200 && rgb_changes > 200);
        for (current, expected) in [(true, &pressed), (false, &before)] {
            let mut m = pollster::block_on(r.begin_movie(size, current)).unwrap();
            m.frame(1.25, 0., |bytes| {
                assert_eq!(bytes, expected);
                Ok(())
            })
            .unwrap();
            m.set_slide_pressure(slide as u32, 0., center).unwrap();
        }
        assert_eq!(picture(&mut r), pressed);
        r.select_track(8);
        assert_eq!(picture(&mut r), before);
        r.select_track(7);
        r.set_slide_pressure(slide as u32, 0., center).unwrap();
        assert_eq!(picture(&mut r), before);
    }

    // Known affine rear phase, sampled over the entire source world. Check
    // the production WGSL inverse against independent CPU expected values
    // after acquiring contact then rotating the glass by a quarter turn.
    let [w, h] = r.fields.size;
    for (i, p) in r.fields.data.chunks_exact_mut(2).enumerate() {
        let x = (i % (w as usize)) as f64 + 0.5;
        let y = ((i / w as usize) % (h as usize)) as f64 + 0.5;
        p[0] = (32. + 190. * x / f64::from(w)).round() as u8;
        p[1] = (32. + 190. * y / f64::from(h)).round() as u8;
    }
    r.slots = [None; 2];
    r.set_slide_pressure(0, 1., [0.365, 0.48]).unwrap();
    r.gesture(0, SlideGesture::Rotate, [0.75, 0.5], [0.5, 0.75]);
    picture(&mut r);
    let field = read_rgba16(&r, &r.interpolated, r.field_size());
    let pose = r.tracks[&7].slides[0];
    let [sx, sy, ox, oy] = r.fields.view_to_cache().map(f64::from);
    let [gw, gh] = r.field_size();
    let mut max_error: f64 = 0.;
    for y in (0..gh).step_by(3) {
        for x in (0..gw).step_by(3) {
            let uv = [
                (f64::from(x) + 0.5) / f64::from(gw),
                (f64::from(y) + 0.5) / f64::from(gh),
            ];
            let q = pose.pressure.inverse(pose.rigid_point(uv)).0;
            let pixel = field[(y * gw + x) as usize];
            for (channel, axis) in [(0, q[0] * sx + ox), (1, uv[1] * sy + oy)] {
                max_error =
                    max_error.max((255. * half(pixel[channel]) - (32. + 190. * axis)).abs());
            }
        }
    }
    println!("Rotated pressure registration max error {max_error:.4} of one RG8 code");
    assert!(max_error < 1.0);

    // Uniform isolated rear dye isolates GLSL optical depth from contour motion.
    for p in r.fields.data.chunks_exact_mut(2) {
        p.copy_from_slice(&[255, 0]);
    }
    r.slots = [None; 2];
    r.select_track(9);
    r.materials
        .insert("constant".into(), Material::constant_film());
    r.set_wheel("constant").unwrap();
    r.set_focus(Some(0.)).unwrap();
    let neutral = picture(&mut r);
    r.set_slide_pressure(0, 1., [0.5; 2]).unwrap();
    let lighter = picture(&mut r);
    let at = |x: usize, y: usize| 4 * (y * size[0] as usize + x) + 1;
    let c = at(160, 90);
    let edge = at(160, 131);
    println!(
        "Uniform rear green code, center {} -> {}, annulus {} -> {}",
        neutral[c], lighter[c], neutral[edge], lighter[edge]
    );
    assert!(lighter[c] > neutral[c] + 40);
    assert!(lighter[edge] + 5 < neutral[edge]);
    assert_eq!(lighter[at(2, 2)], neutral[at(2, 2)]);
}

#[test]
#[ignore = "write the approval sequence to a fresh TSS_PRESSURE_PROOF_DIR with software Vulkan"]
fn write_pressure_performance() {
    let dir = std::path::PathBuf::from(
        std::env::var_os("TSS_PRESSURE_PROOF_DIR").expect("fresh proof directory"),
    );
    let size = [512, 288];
    let mut r = crate::feedback_test::fixture(size);
    r.set_wheel("violet-blue").unwrap();
    let neutral = picture(&mut r);
    for i in 0..96 {
        let k = i % 48;
        let slide = i / 48;
        let a = match k {
            0..=3 => 0.,
            4..=19 => (k - 3) as f32 / 16.,
            20..=27 => 1.,
            28..=43 => (43 - k) as f32 / 16.,
            _ => 0.,
        };
        let load = a * a * (3. - 2. * a);
        let center = if slide == 0 {
            [0.365, 0.48]
        } else {
            [0.58, 0.41]
        };
        r.set_slide_pressure(slide, load, center).unwrap();
        let frame = picture(&mut r);
        if load == 0. {
            assert_eq!(frame, neutral);
        }
        write_ppm(&dir.join(format!("frame-{i:03}.ppm")), size, &frame);
    }
}

#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn native_shader_integrates_dye_and_carrier_across_the_sealed_world() {
    let mut r = crate::feedback_test::fixture([64, 36]);
    let dims = [1408, 1408];
    let output = Target::new(&r.device, "Full glass phase volumes", dims, FIELD_FORMAT);
    let vert = shader(
        &r.device,
        "Fullscreen triangle",
        include_str!("vertex.wgsl"),
    );
    let frag = r.device.create_shader_module(gpu::ShaderModuleDescriptor {
        label: Some("Actual GLSL pressure and coverage operators"),
        source: gpu::util::make_spirv(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/pressure-volume-test.spv"
        ))),
    });
    use Binding::{Buffer, Image, Sampler};
    let image = Image(gpu::TextureViewDimension::D2);
    let pass = pipeline(
        &r.device,
        &vert,
        &frag,
        "main",
        "Phase inventory",
        FIELD_FORMAT,
        &[
            image,
            Sampler,
            Buffer,
            Image(gpu::TextureViewDimension::D2Array),
            image,
            Buffer,
        ],
    );
    let measure = |r: &mut LiveWheel| {
        r.render_offscreen(1.25).unwrap();
        let bindings = bind(
            &r.device,
            &pass,
            &[
                gpu::BindingResource::TextureView(&r.interpolated.view),
                gpu::BindingResource::Sampler(&r.sampler),
                r.material_uniform.as_entire_binding(),
                gpu::BindingResource::TextureView(&r.cache_view),
                gpu::BindingResource::TextureView(&r.zero.view),
                r.pair_uniform.as_entire_binding(),
            ],
        );
        let mut enc = r.device.create_command_encoder(&Default::default());
        draw(&mut enc, "Inventory", &output.view, &pass, &bindings);
        r.queue.submit(Some(enc.finish()));
        let mut sum = [0.; 4];
        for p in read_rgba16(r, &output, dims) {
            for (s, x) in sum.iter_mut().zip(p) {
                *s += half(x);
            }
        }
        sum
    };
    let neutral = measure(&mut r);
    for (slide, center) in [[0.365, 0.48], [0.58, 0.41]].into_iter().enumerate() {
        r.set_slide_pressure(slide as u32, 1., center).unwrap();
        let after = measure(&mut r);
        let errors: [f64; 4] = std::array::from_fn(|i| 100. * (after[i] / neutral[i] - 1.));
        println!("GLSL full-world phase volume changes (%): {errors:.7?}");
        assert!(errors.iter().all(|v| v.abs() < 0.005));
        assert_eq!(errors[2 * (1 - slide)], 0.);
        assert_eq!(errors[2 * (1 - slide) + 1], 0.);
        r.set_slide_pressure(slide as u32, 0., center).unwrap();
    }
}
