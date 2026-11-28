//! Interactive, per-track film sources. Presentation takes only completed CPU
//! snapshots. Basilisk and the native CImg distance transform run on a worker.
use super::{extent, gpu, upload_frame, CellLayout, LiveSolverStatus};
use std::{
    collections::VecDeque,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread,
};
use tss_basilisk_live::{optics::Atlas, Cancellation, Cell, Drive, Slide};

const LEVEL: u32 = 5;
const SIDE: u32 = 176;
const HZ: f64 = 24.;
pub(super) const VIEW: [f32; 4] = [32. / 44., 18. / 44., 6. / 44., 13. / 44.];

#[derive(Clone)]
struct Frame {
    tick: u64,
    rg: Arc<[u8]>,
}

#[derive(Default)]
struct Mailbox {
    stop: bool,
    target_tick: u64,
    pending: [Drive; 2],
    children: Vec<Cancellation>,
    frames: VecDeque<Frame>,
    shown: f64,
    rate: f64,
    last_time: Option<f64>,
    error: Option<String>,
}

impl Mailbox {
    fn tug(&mut self, cell: usize, from: [f64; 2], to: [f64; 2], strength: f32) {
        let world = |v: [f64; 2]| {
            [
                v[0] * f64::from(VIEW[0]) + f64::from(VIEW[2]),
                v[1] * f64::from(VIEW[1]) + f64::from(VIEW[3]),
            ]
        };
        let a = world(from);
        let b = world(to);
        let p = &mut self.pending[cell];
        p.x = ((a[0] + b[0]) * 0.5).clamp(0., 1.);
        p.y = ((a[1] + b[1]) * 0.5).clamp(0., 1.);
        // Heavy and light use the same relative response range as SlidePose.
        let response = (0.13 + 0.29 * f64::from(strength)) / 0.42;
        p.dx = (p.dx + (b[0] - a[0]) * response).clamp(-1., 1.);
        p.dy = (p.dy + (b[1] - a[1]) * response).clamp(-1., 1.);
        p.drive = 1.;
    }

    fn set_rate(&mut self, rate: f64) {
        if self.rate != rate && (self.rate == 0. || rate == 0.) {
            self.last_time = None;
            if rate == 0. {
                self.target_tick = self.shown.ceil() as u64;
            }
        }
        self.rate = rate;
    }

    fn sample(&mut self, time: f64) -> Option<([Frame; 2], f32)> {
        let newest = self.frames.back()?.tick;
        let dt = self
            .last_time
            .replace(time)
            .map_or(0., |t| (time - t).max(0.));
        // Discard time that the worker could not supply. Publishing late fields
        // must never move a held image or cause a catch-up burst on the next draw.
        self.shown = (self.shown + dt * self.rate * HZ).min(newest as f64);
        let first = self.shown.floor() as u64;
        let fraction = (self.shown - first as f64) as f32;
        while self.frames.front()?.tick < first {
            self.frames.pop_front();
        }
        // Keep the upper interpolation endpoint and prime the next interval.
        // The worker can publish at most three ticks beyond the displayed one.
        self.target_tick = if self.rate == 0. {
            self.shown.ceil() as u64
        } else {
            (self.shown + 1. + self.rate.min(2.)).floor() as u64
        };
        let a = &self.frames[0];
        let b = if fraction == 0. { a } else { &self.frames[1] };
        Some(([a.clone(), b.clone()], fraction))
    }
}

type Shared = Arc<(Mutex<Mailbox>, Condvar)>;

fn solve(executable: &Path, seed: u32, layout: CellLayout, shared: &Shared) -> io::Result<()> {
    let start = |slide| {
        Cell::start_registered(executable, slide, seed, LEVEL, |child| {
            let mut state = shared.0.lock().unwrap();
            if state.stop {
                child.cancel();
            } else {
                state.children.push(child);
            }
        })
    };
    let atlas = Atlas::new(SIDE as usize)?;
    let (mut rear, mut rear_field) = start(Slide::Violet)?;
    let mut front = match layout {
        CellLayout::Single => None,
        CellLayout::Paired => Some(start(Slide::Cyan)?),
    };
    loop {
        let mut rg = vec![0; atlas.byte_len()];
        match &front {
            Some((_, field)) => atlas.pack(&rear_field, field, &mut rg)?,
            None => atlas.pack_cell(&rear_field, &mut rg)?,
        }
        let current = Frame {
            tick: rear_field.tick,
            rg: rg.into(),
        };
        let drives = {
            let mut state = shared.0.lock().unwrap();
            if state.stop {
                break;
            }
            let tick = current.tick;
            state.frames.push_back(current);
            state = shared
                .1
                .wait_while(state, |s| !s.stop && s.target_tick <= tick)
                .unwrap();
            if state.stop {
                break;
            }
            std::mem::take(&mut state.pending)
        };
        rear_field = rear.step(drives[0])?;
        if let Some((cell, field)) = &mut front {
            *field = cell.step(drives[1])?;
        }
    }
    Ok(())
}

pub(super) struct Source {
    pub view: gpu::TextureView,
    texture: gpu::Texture,
    uploaded: [Option<u64>; 2],
    shared: Shared,
    pub executable: PathBuf,
    pub seed: u32,
    pub layout: CellLayout,
    latched: Option<(u64, ([u32; 2], f32))>,
}

impl Source {
    pub fn new(
        d: &gpu::Device,
        executable: &Path,
        seed: u32,
        rate: f64,
        layout: CellLayout,
    ) -> Result<Self, String> {
        if !executable.is_file() {
            return Err(format!(
                "Live Basilisk executable is unavailable at {}",
                executable.display()
            ));
        }
        let shared = Shared::default();
        shared.0.lock().unwrap().set_rate(rate);
        let thread_shared = shared.clone();
        let path = executable.to_path_buf();
        thread::Builder::new()
            .name("tss-live-slide".into())
            .spawn(move || {
                if let Err(err) = solve(&path, seed, layout, &thread_shared) {
                    let mut s = thread_shared.0.lock().unwrap();
                    if !s.stop {
                        s.error = Some(err.to_string());
                    }
                }
            })
            .map_err(|err| format!("Starting live Basilisk worker: {err}"))?;
        let texture = d.create_texture(&gpu::TextureDescriptor {
            label: Some("Adjacent solved oil fields"),
            size: extent([SIDE; 2], 2),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::Rg8Unorm,
            usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&gpu::TextureViewDescriptor {
            dimension: Some(gpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        Ok(Self {
            view,
            texture,
            uploaded: [None; 2],
            shared,
            executable: executable.to_owned(),
            seed,
            layout,
            latched: None,
        })
    }

    pub fn status(&self) -> LiveSolverStatus {
        let s = self.shared.0.lock().unwrap();
        LiveSolverStatus {
            state: if s.error.is_some() {
                "failed"
            } else if s.frames.is_empty() {
                "starting"
            } else {
                "ready"
            },
            tick: s.frames.back().map_or(0, |frame| frame.tick),
            flow_rate: s.rate,
            error: s.error.clone(),
        }
    }

    #[cfg(test)]
    pub(super) fn displayed_tick(&self) -> f64 {
        self.shared.0.lock().unwrap().shown
    }

    pub fn set_rate(&mut self, rate: f64) {
        self.shared.0.lock().unwrap().set_rate(rate);
    }

    pub fn tug(&mut self, slide: usize, from: [f64; 2], to: [f64; 2], strength: f32) {
        let cell = match self.layout {
            CellLayout::Single => 0,
            CellLayout::Paired => slide,
        };
        self.shared.0.lock().unwrap().tug(cell, from, to, strength);
    }

    /// Every use of a film within one presentation (including matte passes)
    /// sees the same two states even if the solver publishes mid-composite.
    pub fn present(&mut self, q: &gpu::Queue, presentation: u64, time: f64) -> ([u32; 2], f32) {
        if let Some((previous, frame)) = self.latched {
            if previous == presentation {
                return frame;
            }
        }
        let frame = self.prepare(q, time);
        self.latched = Some((presentation, frame));
        frame
    }

    fn prepare(&mut self, q: &gpu::Queue, time: f64) -> ([u32; 2], f32) {
        let (pair, fraction) = {
            let mut s = self.shared.0.lock().unwrap();
            let Some(sample) = s.sample(time) else {
                return ([0; 2], 0.);
            };
            self.shared.1.notify_one();
            sample
        };
        let slots = pair.each_ref().map(|sample| {
            let slot = (sample.tick % 2) as usize;
            if self.uploaded[slot] != Some(sample.tick) {
                upload_frame(q, &self.texture, [SIDE; 2], slot as u32, &sample.rg);
                self.uploaded[slot] = Some(sample.tick);
            }
            slot as u32
        });
        (slots, fraction)
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        let children = {
            let mut state = self.shared.0.lock().unwrap();
            state.stop = true;
            self.shared.1.notify_one();
            std::mem::take(&mut state.children)
        };
        for child in children {
            child.cancel();
        }
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    fn publish(state: &mut Mailbox, tick: u64) {
        state.frames.push_back(Frame {
            tick,
            rg: Arc::from([]),
        });
    }

    fn sample(state: &mut Mailbox, time: f64) -> f64 {
        let ([first, second], fraction) = state.sample(time).unwrap();
        assert!(second.tick == first.tick || second.tick == first.tick + 1);
        let phase = first.tick as f64 + (second.tick - first.tick) as f64 * f64::from(fraction);
        assert!((phase - state.shown).abs() < 1e-6);
        phase
    }

    #[test]
    fn solved_fields_interpolate_at_the_requested_rate_across_tick_boundaries() {
        for rate in [0., 0.25, 0.5, 1., 1.5, 2.] {
            let mut state = Mailbox::default();
            state.set_rate(rate);
            publish(&mut state, 0);
            for frame in 0..=240 {
                let time = frame as f64 / 60.;
                assert!((sample(&mut state, time) - time * HZ * rate).abs() < 1e-6);
                // The worker delivers all requested ticks before the next draw.
                while state.frames.back().unwrap().tick < state.target_tick {
                    let next = state.frames.back().unwrap().tick + 1;
                    publish(&mut state, next);
                }
                assert!(state.frames.len() <= 4);
            }
        }
    }

    #[test]
    fn fractional_pause_survives_worker_publication_and_resumes_from_the_same_pose() {
        let mut state = Mailbox::default();
        state.set_rate(1.);
        publish(&mut state, 0);
        sample(&mut state, 0.);
        publish(&mut state, 1);
        assert_eq!(sample(&mut state, 0.5 / HZ), 0.5);
        state.set_rate(0.);
        assert_eq!(state.target_tick, 1);
        publish(&mut state, 2); // Already in flight when the user paused.
        for time in [0.5 / HZ, 1., 60.] {
            assert_eq!(sample(&mut state, time), 0.5);
        }
        state.set_rate(1.);
        assert_eq!(sample(&mut state, 90.), 0.5);
        assert!((sample(&mut state, 90. + 0.25 / HZ) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn slow_worker_holds_latest_field_without_skipping_when_it_recovers() {
        let mut state = Mailbox::default();
        state.set_rate(2.);
        publish(&mut state, 0);
        sample(&mut state, 0.);
        assert_eq!(sample(&mut state, 60.), 0.);
        assert_eq!(state.target_tick, 3);
        for tick in 1..=3 {
            publish(&mut state, tick);
        }
        assert_eq!(sample(&mut state, 60.), 0.);
        assert!((sample(&mut state, 60. + 1. / 60.) - 0.8).abs() < 1e-6);
    }

    #[test]
    #[ignore = "requires the native Basilisk executable in TSS_LIVE_SOLVER"]
    fn independent_cells_evolve_and_a_heavy_tug_changes_only_its_cell() -> io::Result<()> {
        use std::time::{Duration, Instant};
        let exe = PathBuf::from(std::env::var_os("TSS_LIVE_SOLVER").expect("Set TSS_LIVE_SOLVER"));
        let run = |seed: u32, tug: bool| -> io::Result<Vec<Frame>> {
            let shared = Shared::default();
            {
                let mut state = shared.0.lock().unwrap();
                state.target_tick = 8;
                if tug {
                    state.tug(0, [0.46, 0.5], [0.60, 0.56], 0.0);
                }
            }
            let worker = thread::scope(|scope| {
                let worker = scope.spawn(|| solve(&exe, seed, CellLayout::Single, &shared));
                let start = Instant::now();
                while !worker.is_finished() && start.elapsed() < Duration::from_secs(10) {
                    if shared.0.lock().unwrap().frames.len() == 9 {
                        break;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
                let mut state = shared.0.lock().unwrap();
                state.stop = true;
                if state.frames.len() != 9 {
                    for child in &state.children {
                        child.cancel();
                    }
                }
                shared.1.notify_one();
                drop(state);
                worker.join().expect("solver thread panicked")
            });
            worker?;
            let mut state = shared.0.lock().unwrap();
            assert_eq!(state.children.len(), 1, "one solver per dyed layer");
            assert_eq!(
                state.frames.len(),
                9,
                "initial field and eight solved steps"
            );
            let frames: Vec<_> = state.frames.drain(..).collect();
            assert!(frames
                .iter()
                .all(|f| f.rg.chunks_exact(2).all(|t| t[1] == 0)));
            assert_ne!(
                frames[0].rg, frames[8].rg,
                "the oil evolves without pointer input"
            );
            Ok(frames)
        };
        let seeds = [83724, 83725, 83726];
        let mut initial = Vec::new();
        for (index, seed) in seeds.into_iter().enumerate() {
            let control = run(seed, false)?;
            assert!(initial.iter().all(|other| other != &control[0].rg));
            initial.push(control[0].rg.clone());
            let performed = run(seed, index == 1)?;
            assert_eq!(performed[0].rg, control[0].rg);
            let changed = performed[8]
                .rg
                .iter()
                .zip(control[8].rg.iter())
                .filter(|(a, b)| a != b)
                .count();
            eprintln!("cell {index}: {changed} signed-distance samples changed at step 8");
            assert_eq!(changed > 0, index == 1);
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
#[ignore = "requires one reserved hardware GPU, a live solver and a new capture directory"]
fn capture_evolving_pair() -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        env, fs,
        io::Write,
        time::{Duration, Instant},
    };
    let worker = env::var_os("TSS_LIVE_SOLVER").ok_or("Set TSS_LIVE_SOLVER")?;
    let out = PathBuf::from(env::var_os("TSS_LIVE_CAPTURE").ok_or("Set TSS_LIVE_CAPTURE")?);
    fs::create_dir(&out)?;
    let (mut renderer, info) = crate::coral_capture::discrete_renderer([384, 216])?;
    renderer.set_preview_width(384)?;
    renderer.select_track(9);
    renderer.set_live_solver(9, Path::new(&worker), 83724, 1.)?;
    let wait_for = |r: &crate::LiveWheel, tick: u64| -> Result<(), String> {
        let start = Instant::now();
        loop {
            let status = r.live_solver_status(9).ok_or("lost track")?;
            if let Some(err) = status.error {
                return Err(err);
            }
            if status.state == "ready" && status.tick >= tick {
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(8) {
                return Err(format!(
                    "timeout at tick {} waiting for {tick}",
                    status.tick
                ));
            }
            thread::sleep(Duration::from_millis(2));
        }
    };
    wait_for(&renderer, 0)?;
    let mut raw = io::BufWriter::new(fs::File::create_new(out.join("projection.rgba"))?);
    let mut first = Vec::new();
    let mut last = Vec::new();
    let frames = 60;
    for i in 0..frames {
        if (8..22).contains(&i) {
            let p = [0.46 + (i - 8) as f32 * 0.012, 0.50 + (i - 8) as f32 * 0.005];
            renderer.gesture(1, crate::SlideGesture::Tug, p, [p[0] + 0.012, p[1] + 0.005]);
        }
        if i == 22 {
            renderer.end_gesture(1);
        }
        if i == 30 {
            let before = renderer.evolving[&9].shared.clone();
            renderer.set_wheel_palette("violet-blue", None)?;
            renderer.set_live_solver(9, Path::new(&worker), 83724, 1.)?;
            assert!(Arc::ptr_eq(&before, &renderer.evolving[&9].shared));
        }
        // Move the optical plane from the rear toward the front while the
        // cells continue to advect after the front contact is released.
        renderer.set_focus(Some((i as f32 / (frames - 1) as f32).clamp(0., 1.)))?;
        let t = i as f64 / 24. + if i == 0 { 0. } else { 1e-6 };
        // A paused or backward-seeking song must not stop physical wheel time.
        let song_time = if i < 30 { 7. } else { 1. };
        renderer.render_film_times_offscreen(t, t, song_time)?;
        wait_for(&renderer, i + 1)?;
        renderer.render_film_times_offscreen(t, t, song_time)?;
        let (w, h, rgba) = renderer.capture_rgba8()?;
        assert_eq!((w, h), (384, 216));
        if i == 0 {
            first.clone_from(&rgba);
        }
        if i == frames - 1 {
            last.clone_from(&rgba);
        }
        if i == 30 {
            let mut png =
                png::Encoder::new(fs::File::create_new(out.join("projection.png"))?, w, h);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()?.write_image_data(&rgba)?;
        }
        raw.write_all(&rgba)?;
        // Serialize readbacks and leave at least one refresh between submissions.
        thread::sleep(Duration::from_millis(18));
    }
    raw.flush()?;
    let changed = first
        .chunks_exact(4)
        .zip(last.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 4))
        .count();
    assert!(changed > 100);
    let t = (frames - 1) as f64 / 24. + 1e-6;
    let snapshot = |r: &mut crate::LiveWheel| -> Result<Vec<u8>, String> {
        r.render_film_times_offscreen(t, t, 1.)?;
        Ok(r.capture_rgba8()?.2)
    };
    let identity = snapshot(&mut renderer)?;
    for finite in [false, true] {
        renderer.set_footprint_sampling(finite);
        let baseline = snapshot(&mut renderer)?;
        renderer.set_detail_scale(9, 0.4)?;
        assert_ne!(snapshot(&mut renderer)? == baseline, true);
        let point = [0.25, 0.70];
        renderer.set_slide_pressure(1, 0.4, point)?;
        let center = renderer.tracks[&9].slides[1].pressure.center;
        assert!((center[0] - 0.40).abs() < 1e-6 && (center[1] - 0.58).abs() < 1e-6);
        renderer.set_slide_pressure(1, 0., point)?;
        renderer.set_detail_scale(9, 1.)?;
        assert_eq!(snapshot(&mut renderer)? == baseline, true);
    }
    renderer.set_footprint_sampling(false);
    assert!(renderer.set_detail_scale(9, f32::NAN).is_err());
    assert_eq!(snapshot(&mut renderer)? == identity, true);

    renderer.set_detail_scale(9, 0.4)?;
    let at = |r: &mut crate::LiveWheel, time| -> Result<Vec<u8>, String> {
        r.render_film_times_offscreen(time, time, 1.)?;
        Ok(r.capture_rgba8()?.2)
    };
    let mut last_zoom = at(&mut renderer, t)?;
    let mut changed_at_zoom = Vec::new();
    for step in 1..=8 {
        let time = t + step as f64 / 96.;
        let image = at(&mut renderer, time)?;
        let changes = image
            .chunks_exact(4)
            .zip(last_zoom.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changes > 0,
            "identical zoomed output at quarter-tick {step}"
        );
        changed_at_zoom.push(changes);
        last_zoom = image;
        wait_for(&renderer, (time * HZ).floor() as u64 + 1)?;
    }
    let held_time = t + 8.5 / 96.;
    let held = at(&mut renderer, held_time)?;
    renderer.set_live_solver(9, Path::new(&worker), 83724, 0.)?;
    assert_eq!(at(&mut renderer, held_time + 30.)? == held, true);
    renderer.set_live_solver(9, Path::new(&worker), 83724, 1.)?;
    assert_eq!(at(&mut renderer, held_time + 60.)? == held, true);
    assert_ne!(at(&mut renderer, held_time + 60. + 1. / 96.)? == held, true);
    println!("detail_scale=0.4 pixel_changes_per_quarter_tick={changed_at_zoom:?} paused_rgba_and_first_resumed_frame_identical=true");
    renderer.set_detail_scale(9, 1.)?;
    renderer.clear_live_solver(9);
    assert_ne!(snapshot(&mut renderer)? == identity, true);
    renderer.set_live_solver(9, Path::new(&worker), 83724, 1.)?;
    wait_for(&renderer, 0)?;
    assert_eq!(renderer.live_solver_status(9).unwrap().tick, 0);
    renderer.forget_track(9);
    assert!(renderer.live_solver_status(9).is_none());
    println!(
        "adapter={} backend={:?} frames={frames} size=384x216 changed_by_more_than_4={changed}",
        info.name, info.backend
    );
    Ok(())
}
