//! CPU-only protocol and phase-volume check. Arguments: solver executable and tree level.
use std::io::Write;
use std::process::{Command, Stdio};

struct Frame {
    time: f64,
    pixels: Vec<u8>,
    oil: f64,
    carrier: f64,
    min_gap: f64,
    max_gap: f64,
}

fn run(exe: &str, level: u32, dye: &str, steps: &[String]) -> Vec<Frame> {
    let mut child = Command::new(exe)
        .args(["--live", dye, "83724", &level.to_string()])
        .env("TSS_LIVE_VOLUME_AUDIT", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for step in steps {
        writeln!(input, "{step}").unwrap();
    }
    writeln!(input, "QUIT").unwrap();
    drop(input);
    let out = child.wait_with_output().unwrap();
    let log = String::from_utf8(out.stderr).unwrap();
    assert!(out.status.success(), "{log}");
    let mut bytes = out.stdout.as_slice();
    let mut frames = Vec::new();
    for line in log.lines().filter(|s| s.starts_with("volume ")) {
        let (header, rest) = bytes.split_at(bytes.iter().position(|&b| b == b'\n').unwrap());
        let fields: Vec<_> = std::str::from_utf8(header)
            .unwrap()
            .split_whitespace()
            .collect();
        assert_eq!(fields[0], "TSSVOF1");
        assert_eq!(fields[1].parse::<usize>().unwrap(), frames.len());
        let width = fields[3].parse::<usize>().unwrap();
        assert_eq!(width, 1 << level);
        let (grid, next) = rest[1..].split_at(width * width);
        bytes = next;
        let values: Vec<f64> = line
            .split_whitespace()
            .skip(1)
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(values[0] as usize, frames.len());
        assert!(
            (values[3] - 1.).abs() < 1e-12,
            "sealed capacity {}",
            values[3]
        );
        assert!((values[1] + values[2] - values[3]).abs() < 1e-14);
        frames.push(Frame {
            time: fields[2].parse().unwrap(),
            pixels: grid.to_vec(),
            oil: values[1],
            carrier: values[2],
            min_gap: values[4],
            max_gap: values[5],
        });
    }
    assert_eq!(frames.len(), steps.len() + 1);
    assert!(bytes.is_empty() || bytes.starts_with(b"\n# Quadtree,"));
    frames
}

fn changed(a: &Frame, b: &Frame) -> (usize, usize) {
    let mut count = 0;
    let mut total = 0;
    for (&p, &q) in a.pixels.iter().zip(&b.pixels) {
        count += usize::from(p != q);
        total += p.abs_diff(q) as usize;
    }
    (count, total)
}

fn drift(frames: &[Frame]) -> (f64, f64) {
    let start = &frames[0];
    frames.iter().fold((0_f64, 0_f64), |(o, c), f| {
        (
            o.max((f.oil / start.oil - 1.).abs()),
            c.max((f.carrier / start.carrier - 1.).abs()),
        )
    })
}

fn near_edge(frame: &Frame, width: usize, aim: (f64, f64)) -> (f64, f64) {
    let target = (
        (aim.0 * width as f64) as usize,
        (aim.1 * width as f64) as usize,
    );
    let i = frame
        .pixels
        .iter()
        .enumerate()
        .filter(|(i, v)| {
            let x = ((i % width) as f64 + 0.5) / width as f64;
            let y = ((i / width) as f64 + 0.5) / width as f64;
            (32..=223).contains(*v) && (0.22..=0.78).contains(&x) && (0.34..=0.66).contains(&y)
        })
        .min_by_key(|(i, _)| {
            (i % width).abs_diff(target.0).pow(2) + (i / width).abs_diff(target.1).pow(2)
        })
        .unwrap()
        .0;
    (
        ((i % width) as f64 + 0.5) / width as f64,
        ((i / width) as f64 + 0.5) / width as f64,
    )
}

fn in_disk(i: usize, width: usize, center: (f64, f64), radius: f64) -> bool {
    let dx = ((i % width) as f64 + 0.5) / width as f64 - center.0;
    let dy = ((i / width) as f64 + 0.5) / width as f64 - center.1;
    dx.hypot(dy) <= radius
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        (3..=4).contains(&args.len()),
        "expected solver path, tree level and optional mask directory"
    );
    let exe = &args[1];
    let level = args[2].parse().unwrap();
    let width = 1usize << level;
    let reference = run(exe, level, "violet", &[]);
    let (x, y) = near_edge(&reference[0], width, (0.65, 0.38));
    let (mx, my) = near_edge(&reference[0], width, (0.34, 0.63));
    let radius = 0.131;
    let press = |load: f64| format!("PRESS {x} {y} {radius} {load}");
    let base = "STEP 0.5 0.5 0 0 0";
    let moved = format!("PRESS {mx} {my} {radius} 1");
    let fixed = run(
        exe,
        level,
        "violet",
        &[
            press(0.25),
            press(1.),
            press(1.),
            press(0.),
            press(1.),
            moved,
            press(0.),
            press(0.),
        ],
    );
    let light = changed(&fixed[0], &fixed[1]);
    let hard = changed(&fixed[0], &fixed[2]);
    let release = changed(&fixed[3], &fixed[4]);
    assert!(light.0 > 0 && hard.1 > light.1 && release.0 > 0);
    assert_eq!(
        changed(&fixed[2], &fixed[3]).0,
        0,
        "unchanged contact while paused"
    );
    assert_eq!(changed(&fixed[7], &fixed[8]).0, 0, "repeat release");
    let moved_cell_changes: Vec<_> = (0..width * width)
        .filter(|&i| fixed[5].pixels[i] != fixed[6].pixels[i])
        .collect();
    let margin = 1.5 / width as f64;
    let old_disk = |i| in_disk(i, width, (x, y), radius + margin);
    let new_disk = |i| in_disk(i, width, (mx, my), radius + margin);
    assert!((0..width * width)
        .filter(|&i| fixed[0].pixels[i] != fixed[2].pixels[i])
        .all(old_disk));
    assert!(moved_cell_changes
        .iter()
        .all(|&i| old_disk(i) || new_disk(i)));
    assert!(moved_cell_changes
        .iter()
        .any(|&i| new_disk(i) && !old_disk(i)));
    assert!(fixed.iter().all(|f| f.time == 0.));
    assert!(fixed[2].min_gap < fixed[1].min_gap && fixed[2].max_gap > 1.);
    assert_eq!((fixed[4].min_gap, fixed[4].max_gap), (1., 1.));
    let (fixed_o, fixed_c) = drift(&fixed);
    assert!(fixed_o < 1e-12 && fixed_c < 1e-12);

    let control_cmds = vec![base.to_owned(); 30];
    let mut with_press = control_cmds.clone();
    for (i, cmd) in with_press.iter_mut().enumerate().take(20) {
        let load = if i < 5 { (i + 1) as f64 / 5. } else { 1. };
        *cmd = format!("{base} {x} {y} {radius} {load}");
    }
    let control = run(exe, level, "violet", &control_cmds);
    let active = run(exe, level, "violet", &with_press);
    let held = changed(&control[20], &active[20]);
    let later = changed(&control[30], &active[30]);
    assert!(held.0 > 0 && later.0 > 0 && active[30].time == control[30].time);
    let (oil, carrier) = drift(&active);
    assert!(oil < 1e-4 && carrier < 1e-4, "oil {oil}, carrier {carrier}");

    let other = run(exe, level, "cyan", &control_cmds);
    let zero_commands = vec![format!("{base} {x} {y} {radius} 0"); control_cmds.len()];
    let untouched = run(exe, level, "cyan", &zero_commands);
    for (a, b) in other.iter().zip(&untouched) {
        assert_eq!(a.pixels, b.pixels);
    }
    if let Some(dir) = args.get(3) {
        std::fs::create_dir_all(dir).unwrap();
        for (name, index) in [
            ("neutral", 0),
            ("light", 1),
            ("hard", 2),
            ("release", 4),
            ("moved", 6),
        ] {
            let mut data = format!("P5\n{width} {width}\n255\n").into_bytes();
            data.extend_from_slice(&fixed[index].pixels);
            std::fs::write(std::path::Path::new(dir).join(format!("{name}.pgm")), data).unwrap();
        }
    }
    println!("{width}x{width}, contact ({x},{y}), moved to ({mx},{my}), radius {radius}; changed pixels contained in the measured contact disks.");
    println!("Paused press: (changed samples, L1 u8) light {light:?}, hard {hard:?}, release {release:?}; repeated hold byte-identical; time zero.");
    println!("Full load gap min {:.6}, max {:.6}; release min and max 1. Paused relative volume drift oil {fixed_o:.3e}, carrier {fixed_c:.3e}.", fixed[2].min_gap, fixed[2].max_gap);
    println!("Evolving press vs control: at tick 20 {held:?}; 10 ticks after release {later:?}. Max relative drift oil {oil:.3e}, carrier {carrier:.3e}. Untouched cell matched all 31 legacy frames.");
}
