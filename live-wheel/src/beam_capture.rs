//! Review fixture: two real optical cells on projector A. Projector B receives
//! a deterministic, opaque-black test film with a moving pair of white ellipses.
use super::*;
use std::{
    env, fs,
    io::{BufWriter, Write},
    path::PathBuf,
    thread,
    time::Duration,
};

const SIZE: [u32; 2] = [640, 360];
const FILM: u64 = 13;
const OIL: [(u64, u32, f32, u32); 2] = [(11, 0xfb70b2, 0.25, 83724), (12, 0x168dd9, 0.74, 92843)];

fn ghost_track() -> Result<MediaTrack, serde_json::Error> {
    let media = serde_json::from_value(serde_json::json!({
        "assetId":FILM,"durationUs":6_000_000,"still":false,"offsetUs":0,"rate":1,"loop":true,
        "mode":"color","keyColor":[0,1,0],"keyThreshold":0.08,"keySoftness":0.04,
        "gradientA":[1,0,0],"gradientB":[0,0,1],"gradientRate":0.5,"bounce":0,"blur":0,
        "distortion":0,"maskA":null,"maskB":null,"maskOp":"intersect","shape":"none"
    }))?;
    Ok(MediaTrack {
        id: FILM,
        opacity: 1.,
        envelope: None,
        media,
    })
}

fn film_frame(id: u64, pts_us: i64) -> Result<DecodedMediaFrame, String> {
    let [w, h] = [320u32, 180];
    let t = pts_us as f32 * 1e-6;
    let (s, c) = (0.30 + 0.75 * t).sin_cos();
    let (cx, cy) = (0.50 + 0.10 * (1.4 * t).sin(), 0.50 + 0.07 * (1.7 * t).cos());
    let mut pixels = Vec::with_capacity((4 * w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32 - cx;
            let v = ((y as f32 + 0.5) / h as f32 - cy) * h as f32 / w as f32;
            let p = [u * c + v * s, v * c - u * s];
            let ring = |x: f32, y: f32| {
                let radius = ((x / 0.164).powi(2) + (y / 0.052).powi(2)).sqrt();
                (1. - (radius - 1.).abs() / 0.10).clamp(0., 1.)
            };
            let center = (1. - (p[0].hypot(p[1]) - 0.019) / 0.003).clamp(0., 1.);
            let light = center.max(ring(p[0], p[1])).max(ring(p[1], p[0]));
            let value = (255. * light).round() as u8;
            pixels.extend([value, value, value, 255]);
        }
    }
    Ok(DecodedMediaFrame {
        id,
        pts_us,
        width: w,
        height: h,
        rgba: pixels.into(),
    })
}

fn beam(intensity: f32, offset: [f32; 2]) -> BeamSettings {
    BeamSettings {
        enabled: true,
        intensity,
        offset,
        lamp_kelvin: 6500,
        focus: Some(0.5),
    }
}

fn snap(r: &mut LiveWheel, t: f64) -> Result<Vec<u8>, String> {
    r.prepare_media(t, true, [0.; 4], film_frame)?;
    r.render_offscreen(t)?;
    let (w, h, pixels) = r.capture_rgba8()?;
    assert_eq!([w, h], SIZE);
    Ok(pixels)
}

fn linear(s: u8) -> f64 {
    let v = f64::from(s) / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(v: f64) -> u8 {
    let v = v.clamp(0., 1.);
    let v = if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    };
    (255. * v).round() as u8
}
fn centroid(image: &[u8]) -> [f64; 2] {
    let mut moments = [0.; 3];
    for (i, p) in image.chunks_exact(4).enumerate() {
        let light = linear(p[0]);
        moments[0] += light;
        moments[1] += light * (i % SIZE[0] as usize) as f64;
        moments[2] += light * (i / SIZE[0] as usize) as f64;
    }
    [moments[1] / moments[0], moments[2] / moments[0]]
}

fn edge_energy(image: &[u8]) -> f64 {
    image
        .chunks_exact(4)
        .enumerate()
        .filter(|(i, _)| i % SIZE[0] as usize != 0)
        .map(|(i, p)| {
            (0..3)
                .map(|c| (f64::from(p[c]) - f64::from(image[(i - 1) * 4 + c])).powi(2))
                .sum::<f64>()
        })
        .sum()
}

#[test]
#[ignore = "requires a reserved discrete GPU, TSS_LIVE_SOLVER and fresh TSS_LIVE_CAPTURE path"]
fn carousel_550r_optical_gate_focuses_oil_media_and_mattes(
) -> Result<(), Box<dyn std::error::Error>> {
    let worker = PathBuf::from(env::var_os("TSS_LIVE_SOLVER").ok_or("Set TSS_LIVE_SOLVER")?);
    let out = PathBuf::from(env::var_os("TSS_LIVE_CAPTURE").ok_or("Set TSS_LIVE_CAPTURE")?);
    fs::create_dir(&out)?;
    let (mut r, adapter) = crate::coral_capture::discrete_renderer(SIZE)?;
    r.set_preview_width(SIZE[0])?;
    r.set_footprint_sampling(true);
    let mut log = fs::File::create_new(out.join("gate-focus.csv"))?;
    writeln!(log, "source,in_focus_edge_energy,away_edge_energy")?;
    let mut record = |source: &str,
                      sharp: &[u8],
                      soft: &[u8]|
     -> Result<(), Box<dyn std::error::Error>> {
        let (focused, defocused) = (edge_energy(sharp), edge_energy(soft));
        writeln!(log, "{source},{focused},{defocused}")?;
        assert!(
            defocused < 0.98 * focused,
            "{source}: {focused} to {defocused}"
        );
        crate::single_film_capture::write_png(&out.join(format!("{source}-sharp.png")), sharp)?;
        crate::single_film_capture::write_png(&out.join(format!("{source}-defocused.png")), soft)?;
        Ok(())
    };
    let projection = lens::GateLens::for_projector(true);
    assert!((projection.distance(0.5) - 132.614).abs() < 0.002);
    assert_eq!(projection.radius(0.5, 0.5), 0.);
    r.select_track(11);
    r.set_single_cell_solver(11, &worker, 83724, 0.)?;
    crate::single_film_capture::wait_tick(&r, 11, 0)?;
    r.set_film_depth(11, Some(0.))?;
    let swatches = Some([0xffffff, 0x235eb7, 0x235eb7, 0xffffff]);
    r.set_wheel_palette("orchid", swatches)?;
    r.set_slide_visibility(true, false);
    r.set_stack(vec![WheelLayer {
        id: 11,
        wheel: "orchid".into(),
        swatches,
        motion: Default::default(),
        material_motion: Default::default(),
        films: [true, false],
        opacity: 1.,
        envelope: None,
    }])?;
    r.set_projector_lenses(true, false);
    r.set_focus(Some(0.))?;
    let a_sharp = snap(&mut r, 0.)?;
    r.set_focus(Some(1.))?;
    let a_soft = snap(&mut r, 0.)?;
    record("A-oil", &a_sharp, &a_soft)?;
    // Isolate B's film at a known registration offset while A's lens rack moves.
    let offset = [0.0625, 0.05];
    let mut secondary = beam(1., offset);
    r.set_primary_intensity(0.)?;
    r.set_media_scene(vec![ghost_track()?], vec![11, FILM])?;
    r.set_projector_routing(Some(secondary), vec![FILM])?;
    r.set_projector_lenses(false, true);
    let b_sharp = snap(&mut r, 0.)?;
    r.set_focus(Some(0.))?;
    r.set_projector_lenses(true, true);
    assert!(
        b_sharp == snap(&mut r, 0.)?,
        "primary lens influenced secondary projector"
    );
    secondary.focus = Some(0.);
    r.set_projector_routing(Some(secondary), vec![FILM])?;
    let b_soft = snap(&mut r, 0.)?;
    record("B-media", &b_sharp, &b_soft)?;
    let shift = centroid(&b_soft);
    let sharp_center = centroid(&b_sharp);
    assert!((shift[0] - sharp_center[0]).abs() < 0.25 && (shift[1] - sharp_center[1]).abs() < 0.25);
    r.set_projector_lenses(true, false);
    assert!(
        b_sharp == snap(&mut r, 0.)?,
        "unfitted secondary film should be unaffected by lens rack"
    );
    // White artwork carrying a live-oil alpha matte, lit by B and translated together.
    let mut masked = ghost_track()?;
    masked.media.mask_a = Some(11);
    r.set_media_scene(vec![masked], vec![FILM])?;
    r.set_projector_routing(Some(secondary), vec![11, FILM])?;
    r.set_projector_lenses(false, true);
    let white = |id, pts_us| {
        Ok(DecodedMediaFrame {
            id,
            pts_us,
            width: 16,
            height: 9,
            rgba: vec![255; 16 * 9 * 4].into(),
        })
    };
    r.prepare_media(0.1, true, [0.; 4], white)?;
    r.render_offscreen(0.1)?;
    let matte_sharp = r.capture_rgba8()?.2;
    secondary.focus = Some(1.);
    r.set_projector_routing(Some(secondary), vec![11, FILM])?;
    r.render_offscreen(0.1)?;
    let matte_soft = r.capture_rgba8()?.2;
    record("B-oil-matte", &matte_sharp, &matte_soft)?;
    println!(
        "{} {:?}; nominal object {:.3} mm; full-gate defocus from center {:.3} design px; {}",
        adapter.name,
        adapter.backend,
        projection.distance(0.5),
        projection.radius(0.5, 0.),
        out.display()
    );
    Ok(())
}

#[test]
#[ignore = "requires a reserved discrete GPU, TSS_LIVE_SOLVER and a fresh TSS_LIVE_CAPTURE path"]
fn separate_projectors_add_ghost_film_to_live_oil() -> Result<(), Box<dyn std::error::Error>> {
    let worker = PathBuf::from(env::var_os("TSS_LIVE_SOLVER").ok_or("Set TSS_LIVE_SOLVER")?);
    let out = PathBuf::from(env::var_os("TSS_LIVE_CAPTURE").ok_or("Set TSS_LIVE_CAPTURE")?);
    fs::create_dir(&out)?;
    let (mut r, adapter) = crate::coral_capture::discrete_renderer(SIZE)?;
    r.set_preview_width(SIZE[0])?;
    r.set_footprint_sampling(true);
    let mut slides = Vec::new();
    for (id, dye, depth, seed) in OIL {
        r.set_single_cell_solver(id, &worker, seed, 0.)?;
        r.set_film_depth(id, Some(depth))?;
        crate::single_film_capture::wait_tick(&r, id, 0)?;
        slides.push(WheelLayer {
            id,
            wheel: "violet-blue".into(),
            swatches: Some([0xffffff, dye, dye, 0xffffff]),
            motion: Default::default(),
            material_motion: Default::default(),
            films: [true, false],
            opacity: 1.,
            envelope: None,
        });
    }
    r.set_stack(slides)?;
    r.set_media_scene(vec![ghost_track()?], vec![OIL[0].0, OIL[1].0, FILM])?;
    let image = |name: &str, bytes: &[u8]| {
        crate::single_film_capture::write_png(&out.join(format!("{name}.png")), bytes)
    };
    r.set_primary_intensity(0.65)?;
    r.set_projector_routing(Some(beam(0., [0., 0.])), vec![FILM])?;
    let a = snap(&mut r, 0.)?;
    image("a-oil", &a)?;
    r.set_lamp_temperature(2700)?;
    let warm = snap(&mut r, 0.)?;
    let sum =
        |pixels: &[u8], c: usize| pixels.chunks_exact(4).map(|p| u64::from(p[c])).sum::<u64>();
    assert!(sum(&warm, 0) > sum(&a, 0) && sum(&warm, 2) < sum(&a, 2));
    r.set_lamp_temperature(6500)?;
    r.set_focus(Some(0.25))?;
    let refocused = snap(&mut r, 0.)?;
    assert!(
        refocused
            .chunks_exact(4)
            .zip(a.chunks_exact(4))
            .filter(|(x, y)| x != y)
            .count()
            > 500
    );
    r.set_focus(None)?;
    r.set_primary_intensity(0.)?;
    r.set_projector_routing(Some(beam(1., [0., 0.])), vec![FILM])?;
    let b = snap(&mut r, 0.)?;
    image("b-test-film", &b)?;
    r.set_lamp_temperature(2700)?;
    r.set_focus(Some(0.9))?;
    assert!(
        b == snap(&mut r, 0.)?,
        "changing the shuttered primary projector altered B"
    );
    r.set_lamp_temperature(6500)?;
    r.set_focus(None)?;
    let pivot = centroid(&b);
    let offset = [0.0625, 0.05];
    r.set_projector_routing(Some(beam(1., offset)), vec![FILM])?;
    let translated = snap(&mut r, 0.)?;
    let shifted = centroid(&translated);
    let delta = [shifted[0] - pivot[0], shifted[1] - pivot[1]];
    for axis in 0..2 {
        assert!(
            (delta[axis] - f64::from(offset[axis] * SIZE[axis] as f32)).abs() < 1.2,
            "offset {delta:?}"
        );
    }
    r.set_primary_intensity(0.65)?;
    let mut log = fs::File::create_new(out.join("additive.csv"))?;
    writeln!(log,"beam_b_gain,changed_pixels,black_pixels,max_srgb_error,registration_dx_px,registration_dy_px")?;
    for (gain, label) in [(0.08, "ghost-08"), (0.25, "ghost-25")] {
        r.set_projector_routing(Some(beam(gain, [0., 0.])), vec![FILM])?;
        let mix = snap(&mut r, 0.)?;
        image(label, &mix)?;
        let (mut changed, mut black, mut max_error) = (0, 0, 0u8);
        for (i, ((a, b), mix)) in a
            .chunks_exact(4)
            .zip(b.chunks_exact(4))
            .zip(mix.chunks_exact(4))
            .enumerate()
        {
            // The test source's entire support is within the middle 40% at t=0.
            // An 8-bit isolated readback cannot distinguish sub-code light at its boundary.
            let x = i as u32 % SIZE[0];
            if x < SIZE[0] * 3 / 10 || x >= SIZE[0] * 7 / 10 {
                black += 1;
                assert_eq!(b[..3], [0; 3]);
                assert_eq!(a, mix, "black film altered projector A at pixel {i}");
            }
            if a != mix {
                changed += 1;
            }
            for c in 0..3 {
                let expected = srgb(linear(a[c]) + f64::from(gain) * linear(b[c]));
                max_error = max_error.max(expected.abs_diff(mix[c]));
            }
        }
        assert!(
            black > SIZE[0] * SIZE[1] / 2 && changed > 500 && max_error <= 2,
            "black={black} changed={changed} max_error={max_error}"
        );
        writeln!(
            log,
            "{gain},{changed},{black},{max_error},{:.4},{:.4}",
            delta[0], delta[1]
        )?;
    }
    for (id, _, _, seed) in OIL {
        r.set_single_cell_solver(id, &worker, seed, 1.)?;
    }
    let mut raw = BufWriter::new(fs::File::create_new(out.join("projection.rgba"))?);
    for n in 0..72 {
        let t = n as f64 / 24. + 1e-6;
        let gain = 0.08 + (0.25 - 0.08) * n as f32 / 71.;
        r.set_projector_routing(Some(beam(gain, [0., 0.])), vec![FILM])?;
        r.render_offscreen(t)?;
        for (id, _, _, _) in OIL {
            crate::single_film_capture::wait_tick(&r, id, n + 1)?;
        }
        raw.write_all(&snap(&mut r, t)?)?;
        assert_eq!(r.media_sample_pts(FILM), Some((t * 1e6).round() as i64));
        thread::sleep(Duration::from_millis(18));
    }
    raw.flush()?;
    println!(
        "{} {:?} — two projectors, 72 frames, {}",
        adapter.name,
        adapter.backend,
        out.display()
    );
    Ok(())
}
