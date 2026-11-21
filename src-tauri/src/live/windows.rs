use super::{Config, Gesture, GesturePoint, MotionPreset, Phase, Rect, ReplayBatch, Slide, Stats};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use serde_json::json;
use std::{
    cell::RefCell,
    num::NonZeroIsize,
    path::PathBuf,
    ptr::null_mut,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, WebviewWindow};
use tss_live_wheel::{wgpu, LiveWheel, SlideGesture};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::ValidateRect,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture},
        WindowsAndMessaging::*,
    },
};

mod benchmark;
pub(super) use benchmark::pressure_trial;
mod capture;
mod macros;
mod movie;
mod projector;
mod visibility;
use super::VisibilityProof;
use benchmark::Benchmark;

pub(super) fn benchmark_output() -> Option<PathBuf> {
    output_arg("--benchmark-live")
}
fn packet(frame: crate::media::DecodedFrame) -> tss_live_wheel::DecodedMediaFrame {
    tss_live_wheel::DecodedMediaFrame {
        id: frame.id,
        pts_us: frame.pts_us,
        width: frame.width,
        height: frame.height,
        rgba: frame.rgba,
    }
}
fn output_arg(option: &str) -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == option {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

pub(super) fn plate_verification() -> Option<PathBuf> {
    output_arg("--verify-wheel-plates")
}

enum Command {
    PlatePreview(super::plates::Selection, macros::Reply<Vec<u8>>),
    Plates(
        super::plates::Selection,
        PathBuf,
        Option<tss_live_wheel::PrintProfile>,
        macros::Reply<PathBuf>,
    ),
    VerifyVisibility(macros::Reply<VisibilityProof>),
    Projector(bool, u32, macros::Reply<()>),
    Set(Option<Config>),
    Forget(u64),
    Record(bool, macros::Reply<f64>),
    Replay(ReplayBatch, macros::Reply<()>),
    Perform(ReplayBatch, u64, macros::Reply<()>),
    Movie(movie::Job, macros::Reply<()>),
}

#[derive(Default)]
pub struct Host {
    tx: Mutex<Option<Sender<Command>>>,
    shared: Arc<Mutex<Stats>>,
}

impl Host {
    pub async fn plate_preview(
        &self,
        selected: super::plates::Selection,
    ) -> Result<Vec<u8>, String> {
        self.request(|reply| Command::PlatePreview(selected, reply))
            .await
    }
    pub async fn plates(
        &self,
        selected: super::plates::Selection,
        parent: PathBuf,
        profile: Option<tss_live_wheel::PrintProfile>,
    ) -> Result<PathBuf, String> {
        self.request(|reply| Command::Plates(selected, parent, profile, reply))
            .await
    }

    pub fn stats(&self) -> Stats {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set(&self, window: WebviewWindow, config: Option<Config>) -> Result<(), String> {
        if let Some(c) = &config {
            if let Some(f) = &c.feedback {
                f.native().validate()?;
            }
            if c.feedback_clock.as_ref().is_some_and(|a| {
                !a.position.is_finite()
                    || a.position < 0.
                    || !a.at_unix_ms.is_finite()
                    || a.at_unix_ms < 0.
            }) {
                return Err("Invalid feedback transport anchor".into());
            }
            if c.focus.is_some_and(|f| !(0.0..=1.0).contains(&f))
                || !(0.0..=1.0).contains(&c.mouse_pressure)
                || c.tug_response.is_some_and(|v| !(0.0..=1.0).contains(&v))
                || !matches!(c.render_width, 640 | 960)
                || c.layer_id == 0
                || c.rect.width == 0
                || c.rect.height == 0
                || c.rect.width > 16384
                || c.rect.height > 16384
            {
                return Err("Invalid live wheel configuration".into());
            }
        }
        let mut tx = self.tx.lock().map_err(|e| e.to_string())?;
        if tx.is_none() && config.is_some() {
            let parent = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
            let app = window.app_handle().clone();
            let shared = self.shared.clone();
            let (sender, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("TSS native live wheel".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run(parent as HWND, rx, &app, &shared)
                    }))
                    .unwrap_or_else(|payload| {
                        let msg = payload
                            .downcast_ref::<String>()
                            .map(String::as_str)
                            .or_else(|| payload.downcast_ref::<&str>().copied())
                            .unwrap_or("unknown error");
                        Err(format!("Native GPU preview failed: {msg}"))
                    });
                    if let Err(error) = result {
                        let mut stats = shared.lock().unwrap_or_else(|e| e.into_inner());
                        stats.running = false;
                        stats.error = Some(error.clone());
                        if let Some(path) = benchmark_output() {
                            let _ = std::fs::write(path.with_extension("error.txt"), error);
                            app.exit(1);
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            *tx = Some(sender);
        }
        if let Some(sender) = tx.as_ref() {
            if sender.send(Command::Set(config)).is_err() {
                *tx = None;
                return Err(self
                    .stats()
                    .error
                    .unwrap_or_else(|| "Live window stopped".into()));
            }
        }
        Ok(())
    }

    pub fn forget(&self, id: u64) -> Result<(), String> {
        if let Some(tx) = self.tx.lock().map_err(|e| e.to_string())?.as_ref() {
            tx.send(Command::Forget(id)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

enum PointerSample {
    Press([f32; 2]),
    Motion([f32; 2], [f32; 2]),
    Release([f32; 2]),
}

#[derive(Default)]
struct DragInput {
    previous: Option<[f32; 2]>,
    samples: Vec<(u64, PointerSample)>,
    load: super::mouse_pressure::MousePressure,
    pressure_mode: bool,
    scripted: bool,
    recorder: macros::Recorder,
}
impl DragInput {
    fn input(&mut self, phase: Phase, point: [f32; 2], sample: PointerSample) {
        let t = macros::event_ms();
        self.samples.push((t, sample));
        if !self.pressure_mode {
            self.recorder.push(phase, point);
        }
    }
    fn release(&mut self) {
        if let Some(point) = self.previous.take() {
            self.input(Phase::Up, point, PointerSample::Release(point));
        }
    }
}
thread_local! { static MOUSE:RefCell<DragInput>=RefCell::new(DragInput::default()); }

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP => {
            let (x, y) = ((lp as i16) as f32, ((lp >> 16) as i16) as f32);
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let p = [
                (x / rect.right.max(1) as f32).clamp(0., 1.),
                (y / rect.bottom.max(1) as f32).clamp(0., 1.),
            ];
            // SetCapture/ReleaseCapture can synchronously dispatch other messages.
            // Release the RefCell borrow before calling back into user32.
            let scripted = MOUSE.with(|m| {
                let mut m = m.borrow_mut();
                match msg {
                    WM_LBUTTONDOWN => {
                        m.release();
                        m.previous = Some(p);
                        m.input(Phase::Down, p, PointerSample::Press(p));
                    }
                    WM_MOUSEMOVE if wp & 1 != 0 => {
                        if let Some(old) = m.previous {
                            m.previous = Some(p);
                            if old != p {
                                m.input(Phase::Move, p, PointerSample::Motion(old, p));
                            }
                        }
                    }
                    _ => m.release(),
                }
                m.scripted
            });
            if !scripted && msg == WM_LBUTTONDOWN {
                SetCapture(hwnd);
            }
            if !scripted && msg == WM_LBUTTONUP {
                ReleaseCapture();
            }
            0
        }
        WM_CAPTURECHANGED | WM_CANCELMODE => {
            MOUSE.with(|m| m.borrow_mut().release());
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            ValidateRect(hwnd, std::ptr::null());
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

struct Child(HWND);
impl Child {
    fn new(parent: HWND) -> Result<Self, String> {
        let name = wide("TssNativeSlideSurface");
        let title = wide("The Sacred Swirls live liquid surface");
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.hInstance = module;
            wc.lpfnWndProc = Some(window_proc);
            wc.hCursor = LoadCursorW(null_mut(), IDC_CROSS);
            wc.lpszClassName = name.as_ptr();
            if RegisterClassW(&wc) == 0
                && std::io::Error::last_os_error().raw_os_error() != Some(1410)
            {
                return Err(format!(
                    "Register live window: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let hwnd = CreateWindowExW(
                0,
                name.as_ptr(),
                title.as_ptr(),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                parent,
                null_mut(),
                module,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return Err(format!(
                    "Create live window: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(Self(hwnd))
        }
    }
    fn place(&self, r: Rect) -> Result<(), String> {
        let ok = unsafe {
            SetWindowPos(
                self.0,
                HWND_TOP,
                r.x,
                r.y,
                r.width as i32,
                r.height as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };
        if ok == 0 {
            Err(format!(
                "Position live window: {}",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

fn pump() {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            macros::posted_time(Some(msg.time));
            DispatchMessageW(&msg);
            macros::posted_time(None);
        }
    }
}

fn consume_mouse(
    engine: &mut LiveWheel,
    slide: Slide,
    gesture: Gesture,
    pressure: f32,
) -> Result<usize, String> {
    let mode = native_mode(gesture);
    MOUSE.with(|m| {
        let mut m = m.borrow_mut();
        let DragInput {
            samples,
            load,
            recorder,
            ..
        } = &mut *m;
        let mut force = Vec::new();
        let mut n = 0;
        for (t, event) in samples.drain(..) {
            match (mode, event) {
                (Some(_), PointerSample::Press(_) | PointerSample::Release(_)) => {
                    engine.end_gesture(slide as u32)
                }
                (Some(mode), PointerSample::Motion(from, to)) => {
                    engine.gesture(slide as u32, mode, from, to);
                    n += 1;
                }
                (None, PointerSample::Press(p)) => {
                    load.input(Phase::Down, p, pressure, t, &mut force)
                }
                (None, PointerSample::Motion(_, p)) => {
                    load.input(Phase::Move, p, pressure, t, &mut force);
                    n += 1;
                }
                (None, PointerSample::Release(p)) => {
                    load.input(Phase::Up, p, pressure, t, &mut force)
                }
            }
        }
        if mode.is_none() {
            load.advance(macros::now_ms(), &mut force);
        }
        if let Some((_, s)) = force.last() {
            engine.set_slide_pressure(slide as u32, s.amount.unwrap(), s.point)?;
        }
        for (t, s) in force {
            recorder.push_at(t, s);
        }
        Ok(n)
    })
}
fn native_mode(gesture: Gesture) -> Option<SlideGesture> {
    match gesture {
        Gesture::Tug => Some(SlideGesture::Tug),
        Gesture::Rotate => Some(SlideGesture::Rotate),
        Gesture::Pressure => None,
    }
}
fn finish_mouse(hwnd: HWND, engine: &mut LiveWheel, c: &Config) -> Result<(), String> {
    MOUSE.with(|m| m.borrow_mut().release());
    consume_mouse(engine, c.slide, c.gesture, c.mouse_pressure)?;
    if let Some(p) = MOUSE.with(|m| {
        let mut m = m.borrow_mut();
        let (t, s) = m.load.detach(macros::now_ms())?;
        m.recorder.push_at(t, s);
        Some(s.point)
    }) {
        engine.set_slide_pressure(c.slide as u32, 0., p)?;
    }
    unsafe {
        if GetCapture() == hwnd {
            ReleaseCapture();
        }
    }
    Ok(())
}
fn percentile(ms: &mut [f64], p: f64) -> f64 {
    if ms.is_empty() {
        return 0.;
    }
    ms.sort_unstable_by(f64::total_cmp);
    ms[((p * ms.len() as f64).ceil() as usize)
        .saturating_sub(1)
        .min(ms.len() - 1)]
}
fn scripted_mouse(hwnd: HWND, rect: Rect, msg: u32, point: [f64; 2]) {
    let x = (point[0] * rect.width as f64) as i32;
    let y = (point[1] * rect.height as f64) as i32;
    let lp = ((y as isize & 65535) << 16) | (x as isize & 65535);
    MOUSE.with(|m| m.borrow_mut().scripted = true);
    unsafe {
        SendMessageW(hwnd, msg, if msg == WM_LBUTTONUP { 0 } else { 1 }, lp);
    }
    MOUSE.with(|m| m.borrow_mut().scripted = false);
}

fn run(
    parent: HWND,
    rx: Receiver<Command>,
    app: &tauri::AppHandle,
    shared: &Mutex<Stats>,
) -> Result<(), String> {
    // Child outlives the Vulkan surface, which is dropped first (reverse declaration order).
    let child = Child::new(parent)?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let raw = Win32WindowHandle::new(NonZeroIsize::new(child.0 as isize).ok_or("Null live HWND")?);
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
            raw_window_handle: RawWindowHandle::Win32(raw),
        })
    }
    .map_err(|e| e.to_string())?;
    let mut engine: Option<LiveWheel> = None;
    let mut config: Option<Config> = None;
    let mut stats = Stats {
        available: true,
        frame_latency: 2,
        ..Default::default()
    };
    let mut previous = None;
    let mut samples = Vec::new();
    let mut report = Instant::now();
    let start = report;
    let footprint_sampling =
        std::env::var("TSS_LIVE_FOOTPRINT").map_or(Ok(true), |v| match v.as_str() {
            "1" => Ok(true),
            "0" => Ok(false),
            _ => Err("TSS_LIVE_FOOTPRINT must be 0 or 1"),
        })?;
    let mut bench = benchmark_output().map(|path| Benchmark::new(path, footprint_sampling));
    let mut capture = output_arg("--capture-live")
        .map(|path| capture::Capture::new(path, Duration::from_secs(8)))
        .transpose()?;
    if bench.is_some() && capture.is_some() {
        return Err("Run motion capture separately from the benchmark".into());
    }
    let mut first_cmd = None;
    let mut playback = std::collections::HashMap::<u64, macros::Playback>::new();
    let mut known_tracks = std::collections::BTreeSet::<u64>::new();
    let mut performers = std::collections::BTreeMap::<u64, macros::Playback>::new();
    let mut release_clock = super::release_clock::ReleaseClock::default();
    let mut last_presented_film_times = None;
    let mut export_pause = Duration::ZERO;
    let mut program_anchor: Option<(Instant, f64, bool)> = None;
    let mut projector: Option<projector::Projector> = None;
    let mut hidden_pacer = visibility::Pacer::default();
    let mut offscreen_submissions = 0u64;
    let mut visibility_probe: Option<visibility::Probe> = None;
    loop {
        pump();
        if visibility_probe
            .as_mut()
            .is_some_and(|probe| probe.poll(offscreen_submissions))
        {
            visibility_probe = None;
        }
        if projector.as_ref().is_some_and(|p| p.closed()) {
            projector = None;
            stats.projector_running = false;
            stats.projector_age_ms = None;
            stats.projector_song_time = None;
            let _ = app.emit("tss-projector-closed", ());
        }
        if let Some(c) = config.as_ref() {
            if MOUSE.with(|m| {
                m.borrow()
                    .samples
                    .iter()
                    .any(|s| matches!(s.1, PointerSample::Press(_)))
            }) {
                if let Some(p) = playback.get_mut(&c.layer_id) {
                    p.physical_press(c.slide, c.gesture);
                }
                if let Some(p) = performers.get_mut(&c.layer_id) {
                    p.physical_press(c.slide, c.gesture);
                }
            }
        }
        if unsafe { IsWindow(parent) } == 0 {
            break;
        }
        for command in first_cmd.take().into_iter().chain(rx.try_iter()) {
            match command {
                Command::VerifyVisibility(reply) => {
                    if let Some(p) = projector.as_ref().filter(|_| visibility_probe.is_none()) {
                        visibility_probe = Some(visibility::Probe::new(parent, p.hwnd(), reply));
                    } else {
                        let _ = reply.try_send(Err(
                            "Open a projector before running visibility verification".into(),
                        ));
                    }
                }
                Command::Projector(enabled, delay, reply) => {
                    let result = if enabled {
                        match engine.as_ref().filter(|_| config.is_some()) {
                            Some(r) => match projector.as_mut() {
                                Some(p) => p.delay(r, delay),
                                None => projector::Projector::open(parent, &instance, r, delay)
                                    .map(|p| projector = Some(p)),
                            },
                            None => {
                                Err("Select a native wheel before opening the projector".into())
                            }
                        }
                    } else {
                        projector = None;
                        Ok(())
                    };
                    stats.projector_running = projector.is_some();
                    stats.projector_age_ms = None;
                    stats.projector_song_time = None;
                    *shared.lock().map_err(|e| e.to_string())? = stats.clone();
                    let _ = reply.try_send(result);
                }
                Command::PlatePreview(selected, reply) => {
                    let result = match (engine.as_mut(), config.as_ref()) {
                        (Some(r), Some(c)) if selected.matches(c) => {
                            let times = last_presented_film_times.unwrap_or_else(|| {
                                release_clock.film_times(
                                    c.layer_id,
                                    c.motion,
                                    c.motion_take,
                                    start.elapsed().saturating_sub(export_pause),
                                )
                            });
                            pollster::block_on(r.plate_framing_preview(times))
                        }
                        _ => Err("Select the wheel again to preview its framing".into()),
                    };
                    let _ = reply.try_send(result);
                }
                Command::Plates(selected, folder, profile, reply) => {
                    let result = match (engine.as_mut(), config.as_ref()) {
                        (Some(r), Some(c)) if selected.matches(c) => {
                            consume_mouse(r, c.slide, c.gesture, c.mouse_pressure)?;
                            macros::flush(app, c)?;
                            let started = Instant::now();
                            let times = last_presented_film_times.unwrap_or_else(|| {
                                release_clock.film_times(
                                    c.layer_id,
                                    c.motion,
                                    c.motion_take,
                                    start.elapsed().saturating_sub(export_pause),
                                )
                            });
                            unsafe {
                                ShowWindow(child.0, SW_HIDE);
                            }
                            let mut stage = String::new();
                            let progress = |name: &str| {
                                pump();
                                if unsafe { IsWindow(parent) == 0 } {
                                    return Err(
                                        "Plate export interrupted when the editor closed".into()
                                    );
                                }
                                if name != stage {
                                    let _ = app.emit("tss-wheel-plate-progress", name);
                                    stage = name.to_owned();
                                }
                                Ok(())
                            };
                            let result = match (selected.print, profile) {
                                (Some(format), Some(profile)) => {
                                    pollster::block_on(r.export_print_mechanical(
                                        &folder, times, format, &profile, progress,
                                    ))
                                }
                                (None, None) => pollster::block_on(r.export_digital_plates(
                                    &folder,
                                    times,
                                    selected.format,
                                    progress,
                                )),
                                _ => Err("Print stock requires its destination ICC profile".into()),
                            };
                            if !c.preview_hidden && unsafe { IsWindow(parent) != 0 } {
                                child.place(c.rect)?;
                            }
                            export_pause += started.elapsed();
                            previous = None;
                            samples.clear();
                            report = Instant::now();
                            result
                        }
                        _ => Err(
                            "Wheel, pose, focus or film selection changed before plate capture"
                                .into(),
                        ),
                    };
                    let _ = reply.try_send(result);
                }
                Command::Movie(job, reply) => {
                    let result = match (engine.as_mut(), config.as_ref()) {
                        (Some(r), Some(c))
                            if c.layer_id == job.request.layer_id
                                && c.stack == job.request.stack
                                && c.wheel == job.request.wheel
                                && c.motion == job.request.motion
                                && c.material_motion == job.request.material_motion
                                && c.visible_films == job.request.visible_films
                                && c.motion_take == job.request.motion_take =>
                        {
                            // Snapshot the operator's current pressure, including a held
                            // contact or an in-flight release, before hiding the preview.
                            consume_mouse(r, c.slide, c.gesture, c.mouse_pressure)?;
                            macros::flush(app, c)?;
                            unsafe {
                                ShowWindow(child.0, SW_HIDE);
                            }
                            let pause_started = Instant::now();
                            let film_times = last_presented_film_times.unwrap_or_else(|| {
                                release_clock.film_times(
                                    c.layer_id,
                                    c.motion,
                                    c.motion_take,
                                    start.elapsed().saturating_sub(export_pause),
                                )
                            });
                            let result = movie::render(r, job, film_times, app, parent);
                            export_pause += pause_started.elapsed();
                            if !c.preview_hidden && unsafe { IsWindow(parent) != 0 } {
                                child.place(c.rect)?;
                            }
                            previous = None;
                            samples.clear();
                            report = Instant::now();
                            result
                        }
                        _ => {
                            Err("Select and show the requested live wheel before exporting".into())
                        }
                    };
                    let _ = reply.try_send(result);
                }
                Command::Record(enabled, reply) => {
                    if let (Some(r), Some(c)) = (engine.as_mut(), config.as_ref()) {
                        consume_mouse(r, c.slide, c.gesture, c.mouse_pressure)?;
                        macros::flush(app, c)?;
                    }
                    let changed = macros::recording() != enabled;
                    let result = macros::record(app, config.as_ref(), enabled);
                    if changed {
                        if let Ok(t) = &result {
                            for (id, p) in &performers {
                                p.signal(
                                    app,
                                    *id,
                                    if enabled { Phase::Down } else { Phase::Up },
                                    *t,
                                )?;
                            }
                        }
                    }
                    let _ = reply.try_send(result);
                }
                Command::Replay(batch, reply) => {
                    let result = if performers
                        .get(&batch.layer_id)
                        .is_some_and(|p| p.occupied(batch.slide, batch.gesture))
                    {
                        if batch.samples.iter().all(|s| matches!(s.phase, Phase::Up)) {
                            Ok(())
                        } else {
                            Err("A performer has this slide control".into())
                        }
                    } else {
                        playback.entry(batch.layer_id).or_default().apply(
                            engine.as_mut(),
                            config.as_ref(),
                            batch,
                        )
                    };
                    let _ = reply.try_send(result);
                }
                Command::Perform(batch, received, reply) => {
                    let result = match (engine.as_mut(), config.as_ref()) {
                        (Some(r), Some(c)) if known_tracks.contains(&batch.layer_id) => {
                            let (id, slide, gesture) = (batch.layer_id, batch.slide, batch.gesture);
                            let down = batch.samples.iter().any(|s| matches!(s.phase, Phase::Down));
                            let result =
                                macros::perform(&mut performers, r, c, batch, received, app);
                            if down && result.is_ok() {
                                if let Some(p) = playback.get_mut(&id) {
                                    p.physical_press(slide, gesture);
                                }
                            }
                            result
                        }
                        _ => Err("Select a native wheel before performing".into()),
                    };
                    let _ = reply.try_send(result);
                }
                Command::Forget(id) => {
                    release_clock.forget(id);
                    playback.remove(&id);
                    performers.remove(&id);
                    known_tracks.remove(&id);
                    if let Some(r) = engine.as_mut() {
                        r.forget_track(id);
                    }
                }
                Command::Set(None) => {
                    if projector.take().is_some() {
                        let _ = app.emit("tss-projector-closed", ());
                    }
                    stats.projector_running = false;
                    stats.projector_age_ms = None;
                    stats.projector_song_time = None;
                    if let (Some(r), Some(c)) = (engine.as_mut(), config.as_ref()) {
                        r.set_feedback(None)?;
                        finish_mouse(child.0, r, c)?;
                        macros::flush(app, c)?;
                        for (id, mut p) in playback.drain() {
                            r.select_track(id);
                            p.release(r)?;
                        }
                        r.select_track(c.layer_id);
                        for (id, p) in std::mem::take(&mut performers) {
                            let mut p = p;
                            if macros::recording() {
                                p.signal(app, id, Phase::Up, macros::now_ms() as f64)?;
                            }
                            r.select_track(id);
                            p.release(r)?;
                        }
                        r.select_track(c.layer_id);
                    }
                    unsafe {
                        ShowWindow(child.0, SW_HIDE);
                    }
                    config = None;
                    program_anchor = None;
                    previous = None;
                    samples.clear();
                    stats.running = false;
                    *shared.lock().map_err(|e| e.to_string())? = stats.clone();
                }
                Command::Set(Some(next)) => {
                    known_tracks.insert(next.layer_id);
                    known_tracks.extend(next.stack.iter().map(|s| s.layer_id));
                    if config.as_ref() == Some(&next) {
                        continue;
                    }
                    if engine.is_none() {
                        child.place(next.rect)?;
                        let fields = tss_live_wheel::bundled_fields()?;
                        let filter = std::env::var("TSS_LIVE_ADAPTER").ok();
                        engine = Some(pollster::block_on(LiveWheel::new(
                            &instance,
                            &surface,
                            &fields,
                            next.rect.width,
                            next.rect.height,
                            filter.as_deref(),
                        ))?);
                        engine
                            .as_mut()
                            .unwrap()
                            .set_footprint_sampling(footprint_sampling);
                    }
                    let r = engine.as_mut().unwrap();
                    if config.as_ref().and_then(|c| c.feedback.as_ref()) != next.feedback.as_ref() {
                        r.set_feedback(next.feedback.as_ref().map(|f| f.native()))?;
                    }
                    if config.as_ref().map(|c| &c.stack) != Some(&next.stack) {
                        r.set_stack(
                            next.stack
                                .iter()
                                .filter(|s| s.media.is_none())
                                .map(|s| tss_live_wheel::WheelLayer {
                                    id: s.layer_id,
                                    wheel: s.wheel.clone(),
                                    motion: s.motion,
                                    material_motion: s.material_motion,
                                    films: s.visible_films.slides(),
                                    opacity: s.opacity,
                                    envelope: s.envelope.clone(),
                                })
                                .collect(),
                        )?;
                        let media = next
                            .stack
                            .iter()
                            .filter_map(|s| {
                                s.media.as_ref().map(|source| tss_live_wheel::MediaTrack {
                                    id: s.layer_id,
                                    opacity: s.opacity,
                                    envelope: s.envelope.clone(),
                                    media: source.clone(),
                                })
                            })
                            .collect();
                        r.set_media_scene(
                            media,
                            next.stack
                                .iter()
                                .filter(|s| !s.matte_only)
                                .map(|s| s.layer_id)
                                .collect(),
                        )?;
                    }
                    let old_clock = config.as_ref().and_then(|c| c.feedback_clock.as_ref());
                    if old_clock != next.feedback_clock.as_ref() {
                        let new_clock = next.feedback_clock.as_ref();
                        if old_clock.map(|a| a.epoch) != new_clock.map(|a| a.epoch) {
                            r.reset_feedback_history();
                            if let Some(p) = projector.as_mut() {
                                p.reset();
                            }
                        }
                        program_anchor = new_clock.map(|a| {
                            let wall_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64()
                                * 1000.;
                            let age = if a.playing {
                                ((wall_ms - a.at_unix_ms) / 1000.).max(0.)
                            } else {
                                0.
                            };
                            (Instant::now(), a.position + age, a.playing)
                        });
                    }
                    if let Some(old) = config.as_ref() {
                        if old.layer_id != next.layer_id
                            || old.slide != next.slide
                            || old.gesture != next.gesture
                            || old.focus != next.focus
                            || old.tug_response != next.tug_response
                            || old.mouse_pressure != next.mouse_pressure
                            || old.rect != next.rect
                        {
                            finish_mouse(child.0, r, old)?;
                            macros::flush(app, old)?;
                            if !next.stack.iter().any(|s| s.layer_id == old.layer_id) {
                                if let Some(mut p) = playback.remove(&old.layer_id) {
                                    p.release(r)?;
                                }
                            }
                        }
                    }
                    let removed: Vec<_> = playback
                        .keys()
                        .copied()
                        .filter(|id| {
                            *id != next.layer_id && !next.stack.iter().any(|l| l.layer_id == *id)
                        })
                        .collect();
                    for id in removed {
                        r.select_track(id);
                        playback.remove(&id).unwrap().release(r)?;
                    }
                    r.select_track(next.layer_id);
                    if next.preview_hidden {
                        unsafe {
                            ShowWindow(child.0, SW_HIDE);
                        }
                    } else if config
                        .as_ref()
                        .is_none_or(|c| c.rect != next.rect || c.preview_hidden)
                    {
                        child.place(next.rect)?;
                    }
                    if config.as_ref().map(|c| c.rect) != Some(next.rect) {
                        r.resize(&surface, next.rect.width, next.rect.height)?;
                    }
                    if config.as_ref().map(|c| c.layer_id) != Some(next.layer_id) {
                        r.select_track(next.layer_id);
                    }
                    if !config.as_ref().is_some_and(|c| {
                        c.layer_id == next.layer_id
                            && c.motion == next.motion
                            && c.material_motion == next.material_motion
                            && c.motion_take == next.motion_take
                    }) {
                        r.set_film_sources(next.motion, next.material_motion)?;
                        last_presented_film_times = None;
                        if next.motion == MotionPreset::CoralRelease
                            && config.as_ref().is_some_and(|old| {
                                old.layer_id == next.layer_id
                                    && old.motion == next.motion
                                    && old.motion_take != next.motion_take
                            })
                        {
                            r.reset_feedback_history();
                        }
                    }
                    if !config.as_ref().is_some_and(|c| {
                        c.layer_id == next.layer_id
                            && c.slide == next.slide
                            && c.tug_response == next.tug_response
                    }) {
                        r.set_tug_response(next.slide as u32, next.tug_response.unwrap_or(0.4))?;
                    }
                    if config.as_ref().map(|c| c.render_width) != Some(next.render_width) {
                        r.set_preview_width(next.render_width)?;
                    }
                    if config.as_ref().map(|c| c.focus) != Some(next.focus) {
                        r.set_focus(next.focus)?;
                    }
                    if config.as_ref().map(|c| (c.layer_id, c.visible_films))
                        != Some((next.layer_id, next.visible_films))
                    {
                        let [rear, front] = next.visible_films.slides();
                        r.set_slide_visibility(rear, front);
                    }
                    if config.as_ref().map(|c| &c.wheel) != Some(&next.wheel) {
                        r.set_wheel(&next.wheel)?;
                    }
                    if !config.as_ref().is_some_and(|old| {
                        old.render_width == next.render_width && old.rect == next.rect
                    }) {
                        previous = None;
                        samples.clear();
                        report = Instant::now();
                    }
                    MOUSE
                        .with(|m| m.borrow_mut().pressure_mode = next.gesture == Gesture::Pressure);
                    stats.adapter = r.adapter_name().to_owned();
                    [stats.width, stats.height] = r.preview_size();
                    stats.layer_id = next.layer_id;
                    stats.focus = next.focus;
                    stats.slide = Some(next.slide);
                    stats.gesture = Some(next.gesture);
                    stats.running = true;
                    config = Some(next);
                    *shared.lock().map_err(|e| e.to_string())? = stats.clone();
                }
            }
        }
        let editor_hidden = config.as_ref().is_some_and(|c| c.preview_hidden)
            || unsafe { IsIconic(parent) != 0 || IsWindowVisible(parent) == 0 };
        if config.is_none() || (editor_hidden && projector.as_ref().is_none_or(|p| !p.visible())) {
            hidden_pacer.clear();
            previous = None;
            samples.clear();
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(c) => first_cmd = Some(c),
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            continue;
        }
        if editor_hidden {
            let wait = hidden_pacer.wait(Instant::now());
            if !wait.is_zero() {
                match rx.recv_timeout(wait.min(Duration::from_millis(25))) {
                    Ok(c) => first_cmd = Some(c),
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                continue;
            }
        } else {
            hidden_pacer.clear();
        }
        let c = config.as_ref().unwrap();
        let r = engine.as_mut().unwrap();
        if let Some(b) = bench.as_mut() {
            b.before(r, child.0, c)?;
        }
        let drags = if c
            .stack
            .iter()
            .any(|l| l.layer_id == c.layer_id && l.media.is_some())
        {
            MOUSE.with(|m| m.borrow_mut().samples.clear());
            0
        } else {
            consume_mouse(r, c.slide, c.gesture, c.mouse_pressure)?
        };
        macros::flush(app, c)?;
        stats.drags += drags as u64;
        let frame_time = start.elapsed().saturating_sub(export_pause);
        for l in c.stack.iter().filter(|l| l.media.is_none()) {
            r.set_layer_times(
                l.layer_id,
                release_clock.film_times(l.layer_id, l.motion, l.motion_take, frame_time),
            )?;
        }
        let film_times = release_clock.film_times(c.layer_id, c.motion, c.motion_take, frame_time);
        let [rear_time, front_time] = film_times;
        let program_time =
            program_anchor.map_or(frame_time.as_secs_f64(), |(instant, pos, playing)| {
                pos + if playing {
                    instant.elapsed().as_secs_f64()
                } else {
                    0.
                }
            });
        stats.error = if c.stack.iter().any(|l| l.media.is_some()) {
            r.prepare_media(
                program_time,
                true,
                crate::media_audio::at(program_time),
                |id, us| crate::media::frame_for_preview(id, us).map(packet),
            )
            .err()
        } else {
            None
        };
        let did_present = if editor_hidden {
            r.render_film_times_offscreen(rear_time, front_time, program_time)?;
            offscreen_submissions += 1;
            false
        } else {
            r.render_film_times(&surface, rear_time, front_time, program_time)?
        };
        if let Some(p) = projector.as_mut() {
            let capture = (did_present || editor_hidden)
                && !c.feedback_clock.as_ref().is_some_and(|a| a.ended);
            match p.tick(r, frame_time.as_secs_f64(), program_time, capture) {
                Ok(Some(t)) => {
                    stats.projector_age_ms = t.age_ms;
                    stats.projector_song_time = t.song_time;
                    stats.projector_presents += 1;
                }
                Ok(None) => (),
                Err(e) => {
                    projector = None;
                    stats.projector_running = false;
                    stats.projector_age_ms = None;
                    stats.projector_song_time = None;
                    let _ = app.emit("tss-projector-closed", e);
                }
            }
        }
        let now = Instant::now();
        if did_present || editor_hidden {
            last_presented_film_times = Some(film_times);
        }
        if did_present {
            stats.presents += 1;
            if let Some(record) = capture.as_mut() {
                if record.frame_at(r, c, frame_time, film_times, stats.drags)? {
                    capture = None;
                }
            }
            if let Some(p) = previous {
                samples.push(now.duration_since(p).as_secs_f64() * 1000.);
            }
            previous = Some(now);
        } else if !editor_hidden {
            std::thread::sleep(Duration::from_millis(2));
        }
        if let Some(b) = bench.as_mut() {
            if b.after(did_present, drags, frame_time, r, child.0, c, app)? {
                drop(engine.take());
                drop(surface);
                drop(child);
                app.exit(0);
                return Ok(());
            }
        }
        if report.elapsed() >= Duration::from_secs(1) {
            stats.fps = if samples.is_empty() {
                0.
            } else {
                samples.len() as f64 * 1000. / samples.iter().sum::<f64>()
            };
            stats.p95_ms = percentile(&mut samples, 0.95);
            if !editor_hidden {
                hidden_pacer.observed(stats.fps);
            }
            samples.clear();
            report = now;
            *shared.lock().map_err(|e| e.to_string())? = stats.clone();
        }
    }
    Ok(())
}
