use super::*;
use serde_json::{json, Value};

fn sample(r: &mut LiveWheel, rear: f64, front: f64) -> Result<Vec<u8>, String> {
    r.render_film_times_offscreen(rear, front, rear)?;
    Ok(r.capture_rgba8()?.2)
}

fn different_pixels(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len());
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| p != q)
        .count()
}

fn movie_matches(r: &mut LiveWheel, rear: f64, front: f64, expected: &[u8]) -> Result<(), String> {
    let size = r.preview_size();
    let mut movie = pollster::block_on(r.begin_movie(size, true))?;
    movie.frame_films(rear, front, rear, |pixels| {
        assert_eq!(
            different_pixels(pixels, expected),
            0,
            "Movie and preview at matched clocks"
        );
        Ok(())
    })
}

fn tug(r: &mut LiveWheel, slide: u32) {
    r.gesture(slide, SlideGesture::Tug, [0.43, 0.57], [0.52, 0.44]);
    r.end_gesture(slide);
}

fn deform(r: &mut LiveWheel, slide: u32) -> Result<(), String> {
    tug(r, slide);
    r.gesture(slide, SlideGesture::Rotate, [0.75, 0.42], [0.71, 0.69]);
    r.set_slide_pressure(slide, 0.95, [0.47, 0.52])
}

fn verify(r: &mut LiveWheel, wheel: &str, finite: bool) -> Result<Value, String> {
    let (rear, front) = (2.635, 0.75);
    r.set_wheel(wheel)?;
    r.set_footprint_sampling(finite);
    let mut solos = Vec::new();
    for (hidden, mask) in [(1, [true, false]), (0, [false, true])] {
        for track in [1, 2] {
            r.forget_track(track);
            r.select_track(track);
            tug(r, 1 - hidden);
        }
        r.set_slide_visibility(true, true);
        let before = sample(r, rear, front)?;
        r.set_slide_visibility(mask[0], mask[1]);
        let solo = sample(r, rear, front)?;
        let removed = different_pixels(&before, &solo);
        assert!(removed > 0);
        r.set_slide_visibility(true, true);
        assert_eq!(
            different_pixels(&before, &sample(r, rear, front)?),
            0,
            "Recombining changed both-film image"
        );
        r.set_slide_visibility(mask[0], mask[1]);
        deform(r, hidden)?;
        let (other_rear, other_front) = if hidden == 0 {
            (5.5, front)
        } else {
            (rear, 3.5)
        };
        let hidden_delta = different_pixels(&solo, &sample(r, other_rear, other_front)?);
        assert_eq!(
            hidden_delta, 0,
            "Hidden clock, tug, rotation or compression changed the solo"
        );
        movie_matches(r, other_rear, other_front, &solo)?;
        let motion_delta = different_pixels(&solo, &sample(r, 5.5, 3.5)?);
        assert!(motion_delta > 0);
        r.set_slide_visibility(true, true);
        let revealed = sample(r, other_rear, other_front)?;
        r.select_track(1);
        deform(r, hidden)?;
        assert_eq!(
            different_pixels(&revealed, &sample(r, other_rear, other_front)?),
            0,
            "Hidden input differs from the same gestures applied while visible"
        );
        movie_matches(r, other_rear, other_front, &revealed)?;
        solos.push(json!({
            "visible":if hidden == 0 { "front" } else { "rear" }, "changed_from_both":removed,
            "changed_with_hidden_clock_and_deformations":hidden_delta,
            "changed_with_visible_clock":motion_delta, "both_restored_exactly":true,
            "gestures_applied_while_hidden_match_visible_replay":true, "movie_matches_solo_and_recombined":true
        }));
    }
    r.set_slide_visibility(false, false);
    let lamp = sample(r, rear, front)?;
    assert_eq!(different_pixels(&lamp, &sample(r, 5.5, 3.5)?), 0);
    movie_matches(r, rear, front, &lamp)?;
    Ok(
        json!({"wheel":wheel, "finite_pixel_setting":finite, "size":r.preview_size(),
        "both_hidden_clock_invariant":true, "both_hidden_movie_matches":true, "solos":solos}),
    )
}

#[test]
#[ignore = "requires a reserved discrete GPU and a new TSS_CORAL_CAPTURE_DIR"]
fn capture_coral_solos() -> Result<(), String> {
    let path =
        PathBuf::from(std::env::var_os("TSS_CORAL_CAPTURE_DIR").ok_or("Missing capture path")?);
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    let (mut r, info) = discrete_renderer([960, 540])?;
    r.set_motion(MotionPreset::WheelWithCoralBeads)?;
    r.set_focus(Some(0.90))?;
    r.set_preview_width(960)?;
    r.set_footprint_sampling(true);
    // This pose and time are the checked-in, full-gate coral-wheel.png reference.
    fs::write(path.join("default-both.rgba"), sample(&mut r, 2.5, 2.5)?)
        .map_err(|e| e.to_string())?;
    let mut checks = Vec::new();
    r.set_preview_width(320)?;
    for wheel in ["violet-blue", "blue-red"] {
        for finite in [false, true] {
            checks.push(verify(&mut r, wheel, finite)?);
        }
    }
    r.select_track(0);
    r.set_preview_width(960)?;
    r.set_wheel("violet-blue")?;
    r.set_footprint_sampling(true);
    let views = [
        ("rear", true, false),
        ("front", false, true),
        ("both", true, true),
    ];
    let mut outputs = Vec::new();
    for (name, rear, front) in views {
        let file =
            File::create_new(path.join(format!("{name}.rgba"))).map_err(|e| e.to_string())?;
        outputs.push((rear, front, BufWriter::new(file)));
    }
    let frames = 96;
    for n in 0..frames {
        let time = f64::from(n) / 24.;
        for (rear, front, file) in &mut outputs {
            r.set_slide_visibility(*rear, *front);
            file.write_all(&sample(&mut r, time, time)?)
                .map_err(|e| e.to_string())?;
        }
    }
    for (_, _, mut file) in outputs {
        file.flush().map_err(|e| e.to_string())?;
    }
    let evidence = json!({
        "adapter":info.name, "backend":format!("{:?}",info.backend), "motion":"wheel-with-coral-beads",
        "wheel":"violet-blue","focus":0.90,"source_size":r.preview_size(),"fps":24,"frames_per_view":frames,
        "views":["rear","front","both"],"film_seconds":"rear = front = frame / 24",
        "finite_pixel_sampling":true,"source_format":"full-range sRGB RGBA8","checks":checks
    });
    fs::write(
        path.join("capture.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
