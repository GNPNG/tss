//! Measure two serial CPU solver steps plus the existing native optical transform.
//! Optional output is an uncompressed TSSDF002 stream consumable after gzip.
use std::{
    env,
    error::Error,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    time::Instant,
};
use tss_basilisk_live::{optics::Atlas, Cell, Drive, Slide};

fn summary(stage: &str, mut ms: Vec<f64>) {
    ms.sort_by(f64::total_cmp);
    let p95 = (95 * ms.len()).div_ceil(100) - 1;
    println!(
        "{stage}_p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
        ms[(ms.len() - 1) / 2],
        ms[p95],
        ms[ms.len() - 1]
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().collect();
    if !(5..=6).contains(&args.len()) {
        return Err("optical_pair WORKER LEVEL ATLAS_SIDE TICKS [NEW_RAW_TSSDF002]".into());
    }
    let level: u32 = args[2].parse()?;
    let atlas = Atlas::new(args[3].parse()?)?;
    let ticks: u32 = args[4].parse()?;
    if !(1..=240).contains(&ticks) {
        return Err("ticks outside 1..=240".into());
    }
    let mut file = args
        .get(5)
        .map(|p| File::create_new(p).map(BufWriter::new))
        .transpose()?;
    if let Some(file) = &mut file {
        file.write_all(b"TSSDF002")?;
        for n in [atlas.side() as u32, atlas.side() as u32, ticks + 1, 24]
            .into_iter()
            .chain(atlas.view())
        {
            file.write_all(&n.to_le_bytes())?;
        }
    }
    let (mut rear, initial_rear) = Cell::start(Path::new(&args[1]), Slide::Violet, 83724, level)?;
    let (mut front, initial_front) = Cell::start(Path::new(&args[1]), Slide::Cyan, 83724, level)?;
    let mut rg = vec![0u8; atlas.byte_len()];
    atlas.pack(&initial_rear, &initial_front, &mut rg)?;
    if let Some(file) = &mut file {
        file.write_all(&rg)?;
    }
    let mut solver_ms = Vec::new();
    let mut optics_ms = Vec::new();
    let mut total_ms = Vec::new();
    println!(
        "level={level} physics_side={} atlas_side={} view={:?} measured_ticks={ticks} seed=83724",
        1u32 << level,
        atlas.side(),
        atlas.view()
    );
    for tick in 1..=ticks {
        let start = Instant::now();
        let rear_field = rear.step(Drive::default())?;
        let front_field = front.step(Drive {
            x: 0.52 + (tick.clamp(3, 8) - 3) as f64 * 0.024,
            y: 0.51,
            dx: 0.024,
            dy: 0.012,
            drive: f64::from((3..=8).contains(&tick) as u8),
        })?;
        let solved = Instant::now();
        atlas.pack(&rear_field, &front_field, &mut rg)?;
        let end = Instant::now();
        solver_ms.push(solved.duration_since(start).as_secs_f64() * 1000.);
        optics_ms.push(end.duration_since(solved).as_secs_f64() * 1000.);
        total_ms.push(end.duration_since(start).as_secs_f64() * 1000.);
        if let Some(file) = &mut file {
            file.write_all(&rg)?;
        }
    }
    if let Some(file) = &mut file {
        file.flush()?;
    }
    summary("two_cells_serial", solver_ms);
    summary("two_channel_distance", optics_ms);
    summary("two_cells_plus_distance", total_ms);
    rear.finish()?;
    front.finish()?;
    Ok(())
}
