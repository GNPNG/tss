//! Compare passive and continuously worked cells on the editor's 64² grid.
use std::path::Path;
use tss_basilisk_live::{Cell, Drive, Field, PlateMotion, Slide, Squeeze};

const HZ: usize = 24;
const SECONDS: usize = 60;

#[derive(Default, Clone, Copy)]
struct Change {
    swept: f64,
    crossings: usize,
    sizeable: usize,
}

fn compare(a: &Field, b: &Field) -> Change {
    let mut d = Change::default();
    // Cell centers inside the centered 32/44 × 18/44 projection gate.
    for y in 19..45 {
        for x in 9..55 {
            let i = 64 * y + x;
            let p = a.coverage[i];
            let q = b.coverage[i];
            let delta = p.abs_diff(q);
            d.swept += f64::from(delta) / 255.;
            d.crossings += usize::from((p >= 128) != (q >= 128));
            d.sizeable += usize::from(delta >= 26);
        }
    }
    d
}

struct Run {
    mean_swept: f64,
    min_swept: f64,
    min_sizeable: usize,
}

fn run(exe: &Path, seed: u32, motion: PlateMotion) -> Run {
    let (mut cell, initial) =
        Cell::start_registered_with_motion(exe, Slide::Violet, seed, 6, motion, |_| {}).unwrap();
    let oil = initial.phase_fraction;
    let (mut max_oil_drift, mut max_carrier_drift) = (0_f64, 0_f64);
    let mut result = Run {
        mean_swept: 0.,
        min_swept: f64::INFINITY,
        min_sizeable: usize::MAX,
    };
    let mut previous = initial;
    for tick in 1..=SECONDS * HZ {
        let f = cell.step(Drive::default()).unwrap();
        max_oil_drift = max_oil_drift.max((f.phase_fraction / oil - 1.).abs());
        max_carrier_drift =
            max_carrier_drift.max(((1. - f.phase_fraction) / (1. - oil) - 1.).abs());
        if tick % HZ == 0 {
            let second = tick / HZ;
            let d = compare(&previous, &f);
            if second >= 31 {
                result.mean_swept += d.swept / (SECONDS - 30) as f64;
                result.min_swept = result.min_swept.min(d.swept);
                result.min_sizeable = result.min_sizeable.min(d.sizeable);
            }
            if [1, 10, 30, 45, 60].contains(&second) {
                println!("{seed} {motion:?} {second}s swept_cells={:.2} half_coverage_crossings={} changes_over_10pct={}", d.swept, d.crossings, d.sizeable);
            }
            previous = f;
        }
    }
    // A flow-speed pause sends no STEP. Contact-only frames retain the same time.
    let center = [0.55, 0.5];
    let full = Squeeze {
        center,
        radius: 0.131,
        load: 1.,
    };
    let pressed = cell.press(full).unwrap();
    let held = cell.press(full).unwrap();
    let released = cell.press(Squeeze { load: 0., ..full }).unwrap();
    assert_eq!(held.coverage, pressed.coverage);
    assert_eq!(released.seconds, previous.seconds);
    assert!((released.phase_fraction - previous.phase_fraction).abs() < 1e-10);
    cell.finish().unwrap();
    println!("{seed} {motion:?} 31..60s mean_swept={:.2} min_swept={:.2} min_changes_over_10pct={} max_relative_oil_drift={max_oil_drift:.7} max_relative_carrier_drift={max_carrier_drift:.7}", result.mean_swept, result.min_swept, result.min_sizeable);
    assert!(max_oil_drift < 0.001 && max_carrier_drift < 0.001);
    result
}

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("expected solver executable");
    for seed in [2430927455, 993332112] {
        let passive = run(Path::new(&path), seed, PlateMotion::AtRest);
        let driven = run(Path::new(&path), seed, PlateMotion::Driven);
        assert!(driven.mean_swept > 3. * passive.mean_swept);
        assert!(driven.min_swept > 8. && driven.min_sizeable >= 12);
    }
}
