use super::{movie::Request as MovieRequest, movie_render};
use super::{release_clock::ReleaseClock, Config, FeedbackAnchor, Gesture, Phase, Stats};
use objc2_app_kit::NSView;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, WebviewWindow};
use tss_live_wheel::{wgpu, DecodedMediaFrame, LiveWheel, MediaTrack, SlideGesture, WheelLayer};

mod proof;
mod view;

enum Command {
    Set(Option<Config>),
    Forget(u64),
    Pointer(Phase, [f32; 2]),
    Movie(
        movie_render::Job,
        tauri::async_runtime::Sender<Result<(), String>>,
    ),
    Quit,
}
type SharedSender = Arc<Mutex<Option<mpsc::Sender<Command>>>>;
type SharedStats = Arc<Mutex<Stats>>;

#[derive(Default)]
pub struct Host {
    sender: SharedSender,
    stats: SharedStats,
}

fn verify_output() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--verify-mac-preview" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

pub(crate) fn watch_verification() {
    if !std::env::args_os().any(|a| a == "--verify-mac-preview") {
        return;
    }
    if verify_output().is_none() {
        eprintln!("--verify-mac-preview requires a new output directory");
        std::process::exit(2);
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(90));
        eprintln!("Mac preview verification timed out before five visible presentations");
        std::process::exit(2);
    });
}

impl Host {
    pub async fn export_movie(
        &self,
        paths: (PathBuf, PathBuf),
        request: MovieRequest,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let (tx, mut rx) = tauri::async_runtime::channel(1);
        self.sender
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .ok_or("Select and show a native wheel before exporting")?
            .send(Command::Movie(
                movie_render::Job {
                    paths,
                    request,
                    cancel,
                },
                tx,
            ))
            .map_err(|_| "The Metal preview worker stopped")?;
        rx.recv().await.ok_or("The Metal preview worker stopped")?
    }
    pub fn stats(&self) -> Stats {
        self.stats.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn forget(&self, id: u64) -> Result<(), String> {
        if let Some(tx) = &*self.sender.lock().map_err(|e| e.to_string())? {
            tx.send(Command::Forget(id)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn set(&self, window: WebviewWindow, config: Option<Config>) -> Result<(), String> {
        if let Some(c) = &config {
            if let Some(f) = &c.feedback {
                f.native().validate()?;
            }
            if c.layer_id == 0
                || !(1..=16384).contains(&c.rect.width)
                || !(1..=16384).contains(&c.rect.height)
                || !matches!(c.render_width, 640 | 960)
                || !(0.0..=1.0).contains(&c.mouse_pressure)
                || c.tug_response.is_some_and(|x| !(0.0..=1.0).contains(&x))
                || c.focus.is_some_and(|x| !(0.0..=1.0).contains(&x))
                || c.feedback_clock.as_ref().is_some_and(|a| {
                    !a.position.is_finite()
                        || a.position < 0.
                        || !a.at_unix_ms.is_finite()
                        || a.at_unix_ms < 0.
                })
            {
                return Err("Invalid live wheel configuration".into());
            }
        }
        let sender = self.sender.clone();
        let shared = self.stats.clone();
        let render_window = window.clone();
        let (reply, rx) = mpsc::sync_channel(1);
        window
            .with_webview(move |webview| {
                // WKWebView inherits NSView and is borrowed only during this main-thread callback.
                let parent = unsafe { &*webview.inner().cast::<NSView>() };
                let result = (|| {
                    let mut tx = sender.lock().map_err(|e| e.to_string())?;
                    if let Some(c) = &config {
                        if let Some((instance, surface)) = view::place(
                            parent,
                            c.rect,
                            sender.clone(),
                            tx.is_none(),
                            c.preview_hidden,
                        )? {
                            *tx = Some(start_worker(
                                instance,
                                surface,
                                render_window,
                                shared,
                                sender.clone(),
                            )?);
                        }
                    } else {
                        view::visibility(parent, true);
                    }
                    if tx
                        .as_ref()
                        .is_some_and(|s| s.send(Command::Set(config)).is_err())
                    {
                        *tx = None;
                        view::visibility(parent, true);
                        return Err("Metal preview worker stopped".into());
                    }
                    Ok(())
                })();
                let _ = reply.send(result);
            })
            .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || rx.recv().map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())?
    }
}

fn start_worker(
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    window: WebviewWindow,
    shared: SharedStats,
    sender: SharedSender,
) -> Result<mpsc::Sender<Command>, String> {
    let (commands, rx) = mpsc::channel();
    let close = commands.clone();
    let destroyed = Arc::new(AtomicBool::new(false));
    let closed = destroyed.clone();
    window.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            closed.store(true, Ordering::Release);
            let _ = close.send(Command::Quit);
        }
    });
    std::thread::Builder::new()
        .name("TSS Metal wheel".into())
        .spawn(move || {
            let output = verify_output();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(
                    &instance,
                    &surface,
                    &window,
                    &rx,
                    &shared,
                    &destroyed,
                    output.as_deref(),
                )
            }))
            .unwrap_or_else(|p| {
                let msg = p
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| p.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown error");
                Err(format!("Metal wheel preview failed: {msg}"))
            });
            if let Err(error) = result {
                let mut stats = shared.lock().unwrap_or_else(|p| p.into_inner());
                stats.running = false;
                stats.error = Some(error.clone());
                drop(stats);
                if output.is_some() {
                    eprintln!("{error}");
                    window.app_handle().exit(1);
                    return;
                }
                let _ = window.with_webview(move |webview| {
                    // A subsequent selection can create a fresh renderer in this view.
                    let parent = unsafe { &*webview.inner().cast::<NSView>() };
                    *sender.lock().unwrap_or_else(|p| p.into_inner()) = None;
                    view::visibility(parent, true);
                });
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(commands)
}

fn export_visibility(window: &WebviewWindow, hidden: bool) -> Result<(), String> {
    window
        .with_webview(move |webview| {
            // Tauri borrows the WKWebView on its AppKit thread.
            let parent = unsafe { &*webview.inner().cast::<NSView>() };
            view::cover_for_export(parent, hidden);
        })
        .map_err(|e| e.to_string())
}

// The AppKit view stays on the UI thread. Only the independently retained
// Metal surface and the renderer enter this worker.
fn run(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
    window: &WebviewWindow,
    rx: &mpsc::Receiver<Command>,
    shared: &SharedStats,
    destroyed: &AtomicBool,
    output: Option<&Path>,
) -> Result<(), String> {
    let mut proof_window = None;
    let mut engine = None;
    let mut active: Option<Config> = None;
    let mut mouse = Mouse::default();
    let mut clock = ReleaseClock::default();
    let mut anchor = None;
    let mut stats = Stats {
        available: true,
        frame_latency: 2,
        ..Default::default()
    };
    let start = Instant::now();
    let mut export_pause = Duration::ZERO;
    let mut last_film_times = None;
    let mut next_frame = start;
    let mut window_check = start;
    let mut window_visible = true;
    let mut last_present = None;
    let mut periods = std::collections::VecDeque::<f64>::with_capacity(61);
    if let Some(dir) = output {
        std::fs::create_dir(dir)
            .map_err(|e| format!("Cannot create verification directory: {e}"))?;
    }
    loop {
        let cmd = if active.as_ref().is_none_or(|c| c.preview_hidden) {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => break,
            }
        } else {
            match rx.recv_timeout(next_frame.saturating_duration_since(Instant::now())) {
                Ok(c) => Some(c),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => None,
            }
        };
        if let Some(command) = cmd {
            match command {
                Command::Quit => break,
                Command::Movie(job, reply) => {
                    let result = match (engine.as_mut(), active.as_ref()) {
                        (Some(r), Some(c))
                            if !c.uses_simulation() && c.layer_id == job.request.layer_id
                                && c.wheel == job.request.wheel
                                && c.detail_scale == job.request.detail_scale
                                && c.swatches == job.request.swatches
                                && c.material_motion == job.request.material_motion
                                && c.motion == job.request.motion
                                && c.focus == job.request.focus
                                && c.lighting == job.request.lighting
                                && c.stack == job.request.stack
                                && c.motion_take == job.request.motion_take
                                && c.visible_films == job.request.visible_films =>
                        {
                            let began = Instant::now();
                            let at = start.elapsed().saturating_sub(export_pause);
                            let times = last_film_times.unwrap_or_else(|| {
                                clock.film_times(c.layer_id, c.motion, c.motion_take, at)
                            });
                            let result = export_visibility(window, true).and_then(|_| {
                                let rendered = objc2::rc::autoreleasepool(|_| {
                                    movie_render::render(r, job, times, window.app_handle(), || {
                                        destroyed.load(Ordering::Acquire)
                                    })
                                });
                                let restored = export_visibility(window, false);
                                rendered.and(restored)
                            });
                            export_pause += began.elapsed();
                            periods.clear();
                            last_present = None;
                            next_frame = Instant::now();
                            result
                        }
                        _ => Err("Wheel or films changed before movie export".into()),
                    };
                    let _ = reply.try_send(result);
                }
                Command::Forget(id) => {
                    if let Some(r) = &mut engine {
                        LiveWheel::forget_track(r, id);
                    }
                    clock.forget(id);
                }
                Command::Pointer(phase, p) => {
                    if let (Some(c), Some(r)) = (&active, &mut engine) {
                        stats.drags += mouse.input(
                            r,
                            c,
                            phase,
                            p,
                            start.elapsed().saturating_sub(export_pause).as_millis() as u64,
                        )? as u64;
                    }
                }
                Command::Set(config) => {
                    if config == active {
                        continue;
                    }
                    if let (Some(r), Some(old)) = (&mut engine, &active) {
                        if config.as_ref().is_none_or(|n| {
                            n.layer_id != old.layer_id
                                || n.slide != old.slide
                                || n.gesture != old.gesture
                                || n.rect != old.rect
                        }) {
                            mouse.release(
                                r,
                                old,
                                start.elapsed().saturating_sub(export_pause).as_millis() as u64,
                            )?;
                        }
                    }
                    if let Some(c) = &config {
                        if active.as_ref().is_none_or(|old| {
                            old.layer_id != c.layer_id
                                || old.wheel != c.wheel
                                || old.material_motion != c.material_motion
                                || old.stack != c.stack
                                || old.motion != c.motion
                                || old.motion_take != c.motion_take
                        }) {
                            last_film_times = None;
                        }
                        if engine.is_none() {
                            let fields = tss_live_wheel::bundled_fields()?;
                            let preferred = std::env::var("TSS_LIVE_ADAPTER").ok();
                            let mut r = objc2::rc::autoreleasepool(|_| {
                                pollster::block_on(LiveWheel::new(
                                    instance,
                                    surface,
                                    &fields,
                                    c.rect.width,
                                    c.rect.height,
                                    preferred.as_deref(),
                                ))
                            })?;
                            r.set_footprint_sampling(true);
                            engine = Some(r);
                        }
                        let r = engine.as_mut().unwrap();
                        objc2::rc::autoreleasepool(|_| configure(r, surface, active.as_ref(), c))?;
                        stats.adapter = r.adapter_name().to_owned();
                        [stats.width, stats.height] = r.preview_size();
                        stats.layer_id = c.layer_id;
                        stats.focus = c.focus;
                        stats.slide = Some(c.slide);
                        stats.gesture = Some(c.gesture);
                    } else if let Some(r) = &mut engine {
                        r.set_feedback(None)?;
                        r.reset_feedback_history();
                    }
                    if active.as_ref().map(|c| (c.rect, c.render_width))
                        != config.as_ref().map(|c| (c.rect, c.render_width))
                    {
                        periods.clear();
                        last_present = None;
                        stats.fps = 0.;
                        stats.p95_ms = 0.;
                    }
                    let next_clock = config.as_ref().and_then(|c| c.feedback_clock.as_ref());
                    if active.as_ref().and_then(|c| c.feedback_clock.as_ref()) != next_clock {
                        let wall_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64()
                            * 1000.;
                        anchor = next_clock.map(|c| {
                            ProgramClock::new(
                                c,
                                wall_ms,
                                start.elapsed().saturating_sub(export_pause),
                            )
                        });
                    }
                    stats.running = config.as_ref().is_some_and(|c| !c.preview_hidden);
                    active = config;
                    *shared.lock().map_err(|e| e.to_string())? = stats.clone();
                    next_frame = Instant::now();
                }
            }
            continue;
        }
        if Instant::now() >= window_check {
            window_visible = window.is_visible().map_err(|e| e.to_string())?
                && !window.is_minimized().map_err(|e| e.to_string())?;
            window_check = Instant::now() + Duration::from_millis(250);
        }
        if !window_visible {
            last_present = None;
            periods.clear();
            next_frame = window_check;
            continue;
        }
        let c = active.as_ref().unwrap();
        let r = engine.as_mut().unwrap();
        let now = start.elapsed().saturating_sub(export_pause);
        mouse.advance(r, c, now.as_millis() as u64)?;
        let program_time = anchor
            .as_ref()
            .map_or(now.as_secs_f64(), |c| c.position(now));
        next_frame = Instant::now() + Duration::from_millis(40);
        let presented = objc2::rc::autoreleasepool(|_| -> Result<bool, String> {
            for l in c.stack.iter().filter(|l| l.media.is_none()) {
                r.set_layer_times(
                    l.layer_id,
                    clock.film_times(l.layer_id, l.motion, l.motion_take, now),
                )?;
            }
            let [rear, front] = clock.film_times(c.layer_id, c.motion, c.motion_take, now);
            stats.error = if c.stack.iter().any(|s| s.media.is_some()) {
                r.prepare_media(
                    program_time,
                    true,
                    crate::media_audio::at(program_time),
                    |id, us| {
                        crate::media::frame_for_preview(id, us).map(|f| DecodedMediaFrame {
                            id: f.id,
                            pts_us: f.pts_us,
                            width: f.width,
                            height: f.height,
                            rgba: f.rgba,
                        })
                    },
                )
                .err()
            } else {
                None
            };
            let presented = r.render_film_times(surface, rear, front, program_time)?;
            if presented {
                last_film_times = Some([rear, front]);
            }
            Ok(presented)
        })?;
        if presented {
            let now = Instant::now();
            stats.presents += 1;
            c.read_solver_status(&mut stats, |id| r.live_solver_status(id));
            if output.is_some() {
                if let Some(error) = &stats.error { return Err(error.clone()); }
            }
            if let Some(last) = last_present.replace(now) {
                periods.push_back(now.duration_since(last).as_secs_f64() * 1000.);
                if periods.len() > 60 {
                    periods.pop_front();
                }
                let mut sorted: Vec<_> = periods.iter().copied().collect();
                sorted.sort_by(f64::total_cmp);
                stats.fps = 1000. * sorted.len() as f64 / sorted.iter().sum::<f64>();
                stats.p95_ms = sorted[(sorted.len() * 95).div_ceil(100) - 1];
            }
            *shared.lock().map_err(|e| e.to_string())? = stats.clone();
            if stats.presents == 2 {
                if let Some(dir) = output {
                    proof_window = Some(proof::inspect(window, dir)?);
                }
            }
            if stats.presents >= 5 && (c.simulation.is_none() || stats.simulation_tick.is_some_and(|tick| tick >= 24)) {
                if let Some(dir) = output {
                    proof::capture(
                        dir,
                        r,
                        &stats,
                        proof_window.ok_or("Missing verification window")?,
                    )?;
                    window.app_handle().exit(0);
                    break;
                }
            }
        }
    }
    Ok(())
}

// Convert the editor's wall-clock timestamp once. Render frames then follow
// the same monotonic clock as the film cache, including between transport updates.
struct ProgramClock {
    at: Duration,
    song: f64,
    playing: bool,
}
impl ProgramClock {
    fn new(c: &FeedbackAnchor, wall_ms: f64, at: Duration) -> Self {
        let age = if c.playing {
            (wall_ms - c.at_unix_ms).max(0.) / 1000.
        } else {
            0.
        };
        Self {
            at,
            song: c.position + age,
            playing: c.playing,
        }
    }
    fn position(&self, now: Duration) -> f64 {
        self.song
            + if self.playing {
                now.saturating_sub(self.at).as_secs_f64()
            } else {
                0.
            }
    }
}

#[derive(Default)]
struct Mouse {
    last: Option<[f32; 2]>,
    pressure: super::mouse_pressure::MousePressure,
}
impl Mouse {
    fn input(
        &mut self,
        r: &mut LiveWheel,
        c: &Config,
        phase: Phase,
        point: [f32; 2],
        ms: u64,
    ) -> Result<bool, String> {
        let previous = self.last;
        match phase {
            Phase::Down => self.last = Some(point),
            Phase::Move if previous.is_some() => self.last = Some(point),
            Phase::Move | Phase::Up if previous.is_none() => return Ok(false),
            Phase::Up => self.last = None,
            _ => (),
        }
        let slide = c.slide as u32;
        if c.gesture == Gesture::Pressure {
            self.pressure
                .input(phase, point, c.mouse_pressure, ms, &mut Vec::new());
        } else {
            if matches!(phase, Phase::Move | Phase::Up) {
                if let Some(from) = previous {
                    let mode = if c.gesture == Gesture::Rotate {
                        SlideGesture::Rotate
                    } else {
                        SlideGesture::Tug
                    };
                    r.gesture(slide, mode, from, point);
                }
            }
            if matches!(phase, Phase::Up) {
                r.end_gesture(slide);
            }
        }
        Ok(matches!(phase, Phase::Move))
    }
    fn advance(&mut self, r: &mut LiveWheel, c: &Config, ms: u64) -> Result<(), String> {
        if c.gesture == Gesture::Pressure {
            if let Some((previous, _)) = self.pressure.current() {
                self.pressure.advance(ms, &mut Vec::new());
                let (point, amount) = self.pressure.current().unwrap_or((previous, 0.));
                r.set_slide_pressure(c.slide as u32, amount, point)?;
            }
        }
        Ok(())
    }
    fn release(&mut self, r: &mut LiveWheel, c: &Config, ms: u64) -> Result<(), String> {
        if let Some((_, sample)) = self.pressure.detach(ms) {
            r.set_slide_pressure(c.slide as u32, 0., sample.point)?;
        }
        if self.last.take().is_some() {
            r.end_gesture(c.slide as u32);
        }
        Ok(())
    }
}

fn configure(
    r: &mut LiveWheel,
    surface: &wgpu::Surface<'_>,
    old: Option<&Config>,
    c: &Config,
) -> Result<(), String> {
    c.configure_simulation(r)?;
    if old.map(|v| &v.stack) != Some(&c.stack) {
        r.set_stack(
            c.stack
                .iter()
                .filter(|s| s.media.is_none())
                .map(|s| WheelLayer {
                    id: s.layer_id,
                    wheel: s.wheel.clone(),
                    swatches: s.swatches,
                    motion: s.motion,
                    material_motion: s.material_motion,
                    films: s.visible_films.slides(),
                    opacity: s.opacity,
                    envelope: s.envelope.clone(),
                })
                .collect(),
        )?;
        let media = c
            .stack
            .iter()
            .filter_map(|s| {
                s.media.as_ref().map(|m| MediaTrack {
                    id: s.layer_id,
                    opacity: s.opacity,
                    envelope: s.envelope.clone(),
                    media: m.clone(),
                })
            })
            .collect();
        r.set_media_scene(
            media,
            c.stack
                .iter()
                .filter(|s| !s.matte_only)
                .map(|s| s.layer_id)
                .collect(),
        )?;
    }
    if old.and_then(|v| v.feedback.as_ref()) != c.feedback.as_ref() {
        r.set_feedback(c.feedback.as_ref().map(|f| f.native()))?;
    }
    if old.and_then(|v| v.feedback_clock.as_ref().map(|a| a.epoch))
        != c.feedback_clock.as_ref().map(|a| a.epoch)
        || old.is_some_and(|v| {
            v.layer_id == c.layer_id && v.motion == c.motion && v.motion_take != c.motion_take
        })
    {
        r.reset_feedback_history();
    }
    if old.map(|v| v.rect) != Some(c.rect) {
        r.resize(surface, c.rect.width, c.rect.height)?;
    }
    if old.map(|v| v.render_width) != Some(c.render_width) {
        r.set_preview_width(c.render_width)?;
    }
    r.select_track(c.layer_id);
    if old.map(|v| (&v.wheel, v.swatches, v.layer_id)) != Some((&c.wheel, c.swatches, c.layer_id)) {
        r.set_wheel_palette(&c.wheel, c.swatches)?;
    }
    if old.map(|v| (v.motion, v.material_motion, v.layer_id))
        != Some((c.motion, c.material_motion, c.layer_id))
    {
        r.set_film_sources(c.motion, c.material_motion)?;
    }
    let [rear, front] = c.visible_films.slides();
    r.set_slide_visibility(rear, front);
    r.set_tug_response(c.slide as u32, c.tug_response.unwrap_or(0.4))?;
    // Returning from a hidden preview re-applies focus, including the default.
    if old.is_none_or(|v| v.focus != c.focus) {
        r.set_focus(c.focus)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_clock_accounts_for_ipc_age_and_holds_the_paused_song_position() {
        let mut clock = FeedbackAnchor {
            epoch: 3,
            position: 2.5,
            at_unix_ms: 1000.,
            playing: true,
            ended: false,
        };
        let at = Duration::from_secs(10);
        assert_eq!(
            ProgramClock::new(&clock, 1250., at).position(at + Duration::from_millis(750)),
            3.5
        );
        assert_eq!(ProgramClock::new(&clock, 750., at).position(at), 2.5);
        clock.playing = false;
        assert_eq!(
            ProgramClock::new(&clock, 1250., at).position(at + Duration::from_secs(20)),
            2.5
        );
    }
}
