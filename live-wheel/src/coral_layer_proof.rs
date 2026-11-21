use crate::warp_diagnostic::read_rgba16;
use crate::{LiveWheel, MotionPreset};
use serde_json::{json, Value};

/// Compare the two half-float distance channels before optics mix their colors.
/// Run in the same device session and at the same gate as the approval capture.
pub(crate) fn verify(renderer: &mut LiveWheel) -> Result<Value, String> {
    fn fields(r: &mut LiveWheel, rear: f64, front: f64) -> Result<Vec<[u16; 4]>, String> {
        r.render_film_times_offscreen(rear, front, rear)?;
        Ok(read_rgba16(r, &r.interpolated, r.field_size()))
    }
    let differences = |a: &[[u16; 4]], b: &[[u16; 4]], channel: usize| {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b)
            .filter(|(x, y)| x[channel] != y[channel])
            .count()
    };

    // The original wheel is the reference at each rear phase, including a
    // fractional sample and the return leg of the original wheel trajectory.
    let phases = [0.0, 1.137, 3.02, 6.25];
    renderer.set_motion(MotionPreset::WheelFlow)?;
    let references = phases
        .iter()
        .map(|&t| fields(renderer, t, t))
        .collect::<Result<Vec<_>, _>>()?;
    renderer.set_motion(MotionPreset::WheelWithCoralBeads)?;
    let mut rear_differences = Vec::new();
    for (&rear, reference) in phases.iter().zip(&references) {
        let mixed = fields(renderer, rear, 0.85)?;
        rear_differences.push(differences(reference, &mixed, 0));
        if differences(reference, &mixed, 1) == 0 {
            return Err("Original and small-release front fields are identical".into());
        }
    }
    if rear_differences.iter().any(|&n| n != 0) {
        return Err(format!(
            "Original rear field changed in the mix: {rear_differences:?}"
        ));
    }

    let held = fields(renderer, 1.137, 30.0)?;
    let continued = fields(renderer, 3.02, 31.0)?;
    let replay = fields(renderer, 3.02, 0.0)?;
    let moving_rear = differences(&held, &continued, 0);
    let held_front = differences(&held, &continued, 1);
    let replay_rear = differences(&continued, &replay, 0);
    let replay_front = differences(&continued, &replay, 1);
    if moving_rear == 0 || held_front != 0 || replay_rear != 0 || replay_front == 0 {
        return Err("Rear motion and front release clocks were not independent".into());
    }

    let rear = 2.635;
    let front = 0.75;
    renderer.render_film_times_offscreen(rear, front, rear)?;
    let (w, h, preview) = renderer.capture_rgba8()?;
    {
        let mut movie = pollster::block_on(renderer.begin_movie([w, h], true))?;
        movie.frame_films(rear, front, rear, |pixels| {
            if pixels == preview.as_slice() {
                Ok(())
            } else {
                Err("Preview and movie produced different pixels at identical film times".into())
            }
        })?;
    }
    renderer.render_film_times_offscreen(0., 0., 0.)?;
    Ok(json!({
        "fieldSize": renderer.field_size(),
        "originalRearComparisonSeconds": phases,
        "rearSamplesDifferentFromOriginal": rear_differences,
        "rearSamplesChangedWithFrontHeld": moving_rear,
        "frontSamplesChangedDuringHold": held_front,
        "rearSamplesChangedOnFrontReplay": replay_rear,
        "frontSamplesChangedOnReplay": replay_front,
        "movieMatchesPreviewExactlyAtRearFrontSeconds": [rear, front],
    }))
}
