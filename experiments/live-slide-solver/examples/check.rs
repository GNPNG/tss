use std::{env, error::Error, path::Path, time::Instant};
use tss_basilisk_live::{Cell, Drive, Field, PlateMotion, Slide};

fn changed(a: &Field, b: &Field) -> (usize, u64) {
    assert_eq!(a.coverage.len(), b.coverage.len());
    let deltas = a
        .coverage
        .iter()
        .zip(&b.coverage)
        .map(|(a, b)| a.abs_diff(*b));
    (
        deltas.clone().filter(|delta| *delta != 0).count(),
        deltas.map(u64::from).sum(),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let exe = args.next().ok_or("missing live solver executable path")?;
    let level = args
        .next()
        .and_then(|v| v.to_str()?.parse().ok())
        .unwrap_or(6);
    let motion = match args.next().as_deref() {
        None => PlateMotion::AtRest,
        Some(flag) if flag == "--driven" => PlateMotion::Driven,
        _ => return Err("expected optional --driven".into()),
    };
    let start = |slide| {
        Cell::start_registered_with_motion(Path::new(&exe), slide, 83724, level, motion, |_| {})
    };
    let (mut front, mut a) = start(Slide::Cyan)?;
    let (mut rear, mut b) = start(Slide::Violet)?;
    let (mut front_control, mut c) = start(Slide::Cyan)?;
    let (mut rear_control, mut d) = start(Slide::Violet)?;
    assert_eq!(a.coverage, c.coverage);
    assert_eq!(b.coverage, d.coverage);
    assert_ne!(a.coverage, b.coverage);
    // Pick a central oil interface on the actual seeded front field.
    let center = (a.width as f64 - 1.) / 2.;
    let point = a
        .coverage
        .iter()
        .enumerate()
        .filter(|(_, &v)| (12..244).contains(&v))
        .min_by(|(i, _), (j, _)| {
            let distance = |n: usize| {
                (n as f64 % a.width as f64 - center).powi(2)
                    + ((n / a.width) as f64 - center).powi(2)
            };
            distance(*i).total_cmp(&distance(*j))
        })
        .ok_or("fixture has no interface")?
        .0;
    let x = ((point % a.width) as f64 + 0.5) / a.width as f64;
    let y = ((point / a.width) as f64 + 0.5) / a.height as f64;
    let (mass_front, mass_rear) = (a.phase_fraction, b.phase_fraction);
    let mut max_drift = [0.0_f64; 2];
    let mut tick_ms = Vec::new();
    let mut previous_release_difference = Vec::new();
    println!("level={level} seed=83724 contact=({x:.6},{y:.6}) front_mass={mass_front:.10} rear_mass={mass_rear:.10}");
    println!("tick,front_changed_samples,front_sum_abs_delta,rear_changed_samples");
    for tick in 1..=18 {
        let driven = (3..=8).contains(&tick);
        let strength = if driven { 1. } else { 0. };
        let command = Drive {
            x: x + (f64::from(tick.clamp(3, 8)) - 3.) * 0.024,
            y,
            dx: 0.024,
            dy: 0.012,
            drive: strength,
        };
        let start = Instant::now();
        a = front.step(command)?;
        tick_ms.push(start.elapsed().as_secs_f64() * 1000.);
        b = rear.step(Drive::default())?;
        c = front_control.step(Drive::default())?;
        d = rear_control.step(Drive::default())?;
        let front_diff = changed(&a, &c);
        let rear_diff = changed(&b, &d);
        println!("{tick},{},{},{}", front_diff.0, front_diff.1, rear_diff.0);
        assert_eq!(rear_diff, (0, 0), "unforced rear cells diverged");
        assert_eq!(b.phase_fraction, d.phase_fraction);
        let expected_time = f64::from(tick) / 24.;
        assert!(
            (a.seconds - expected_time).abs() < 1e-9 && (b.seconds - expected_time).abs() < 1e-9
        );
        for (i, value, reference) in [
            (0, a.phase_fraction, mass_front),
            (0, c.phase_fraction, mass_front),
            (1, b.phase_fraction, mass_rear),
            (1, d.phase_fraction, mass_rear),
        ] {
            max_drift[i] = max_drift[i].max((value / reference - 1.).abs());
        }
        if tick < 3 {
            assert_eq!(front_diff, (0, 0));
        }
        if tick == 8 {
            assert!(front_diff.0 > 2 && front_diff.1 > 20);
        }
        // On release, both front cells see identical zero moving-wall velocity.
        // Check that the difference field itself continues changing.
        if tick >= 9 {
            let difference: Vec<i16> = a
                .coverage
                .iter()
                .zip(&c.coverage)
                .map(|(a, c)| i16::from(*a) - i16::from(*c))
                .collect();
            assert!(front_diff.1 > 20);
            if !previous_release_difference.is_empty() {
                assert_ne!(difference, previous_release_difference);
            }
            previous_release_difference = difference;
        }
    }
    assert!(
        max_drift.iter().all(|d| *d < 1e-4),
        "phase mass drift {max_drift:?}"
    );
    tick_ms.sort_by(f64::total_cmp);
    println!("max_relative_mass_drift_front={:.9e} rear={:.9e} one_cell_tick_median_ms={:.3} max_ms={:.3}",
        max_drift[0],max_drift[1],tick_ms[tick_ms.len()/2],tick_ms[tick_ms.len()-1]);
    front.finish()?;
    rear.finish()?;
    front_control.finish()?;
    rear_control.finish()?;
    Ok(())
}
