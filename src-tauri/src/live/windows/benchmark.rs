// Interactive benchmark uses the editor's existing wheel picker and child window.
// A local HWND stroke exercises normal input handling without moving the OS pointer.
use super::super::BenchmarkConfig;
use super::*;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const RUNS: [(&str, Slide, Gesture); 8] = [
    ("violet-blue", Slide::Rear, Gesture::Tug),
    ("violet-blue", Slide::Rear, Gesture::Rotate),
    ("blue-red", Slide::Front, Gesture::Tug),
    ("yellow-red", Slide::Front, Gesture::Rotate),
    ("blue-green", Slide::Rear, Gesture::Tug),
    ("blue-yellow", Slide::Rear, Gesture::Rotate),
    ("violet-green", Slide::Front, Gesture::Tug),
    ("violet-red", Slide::Front, Gesture::Rotate),
];
const WARM: Duration = Duration::from_secs(2);
const PERIOD: Duration = Duration::from_secs(12);
pub(crate) fn pressure_trial() -> bool {
    std::env::args_os().any(|arg| arg == "--pressure-trial")
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayObservation {
    motion_take: u64,
    requested_ms: f64,
    first_presented_ms: Option<f64>,
    stroke_starts: u64,
    mouse_moves_applied: u64,
}

fn unix_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|e| e.to_string())
}

pub(super) struct Benchmark {
    path: PathBuf,
    footprint_sampling: bool,
    index: usize,
    count: usize,
    settings: BenchmarkConfig,
    initial_take: u64,
    replays: Vec<ReplayObservation>,
    measured_from_unix_ms: Option<u64>,
    pressing: bool,
    began: Option<Instant>,
    stroke_started: Instant,
    motion: Option<capture::Capture>,
    motion_drags: u64,
    last: Option<Instant>,
    intervals: Vec<f64>,
    presents: u64,
    skips: u64,
    drags: u64,
    proof: serde_json::Value,
    results: Vec<serde_json::Value>,
    deadline: Instant,
}

impl Benchmark {
    pub fn new(path: PathBuf, footprint_sampling: bool) -> Self {
        let settings = BenchmarkConfig::current();
        let single = settings.wheel_stack
            || settings.gesture == Gesture::Pressure
            || settings.motion != MotionPreset::WheelFlow;
        Self {
            path,
            footprint_sampling,
            index: 0,
            settings,
            initial_take: 0,
            replays: Vec::new(),
            measured_from_unix_ms: None,
            pressing: false,
            count: if single {
                1
            } else if std::env::args_os().any(|arg| arg == "--gesture-pair") {
                2
            } else {
                RUNS.len()
            },
            began: None,
            stroke_started: Instant::now(),
            motion: None,
            motion_drags: 0,
            last: None,
            intervals: Vec::new(),
            presents: 0,
            skips: 0,
            drags: 0,
            proof: serde_json::Value::Null,
            results: Vec::new(),
            deadline: Instant::now() + Duration::from_secs(45),
        }
    }
    fn pressure_proof(&mut self, r: &mut LiveWheel, hwnd: HWND, c: &Config) -> Result<(), String> {
        let frozen = |r: &mut LiveWheel| -> Result<Vec<u8>, String> {
            r.render_offscreen(0.65)?;
            Ok(r.capture_rgba8()?.2)
        };
        r.select_track(0);
        let neutral = frozen(r)?;
        let changed = |a: &[u8], b: &[u8]| {
            a.chunks_exact(4)
                .zip(b.chunks_exact(4))
                .filter(|(x, y)| (0..3).any(|i| x[i].abs_diff(y[i]) >= 3))
                .count()
        };
        let mut reports = Vec::new();
        for (slide, name, point) in [
            (Slide::Rear, "rear", [0.48, 0.52]),
            (Slide::Front, "front-original", [0.48, 0.52]),
            (Slide::Front, "front", [0.58, 0.41]),
        ] {
            let rec = macros::ProofRecorder::start();
            scripted_mouse(hwnd, c.rect, WM_LBUTTONDOWN, point);
            consume_mouse(r, slide, Gesture::Pressure, c.mouse_pressure)?;
            std::thread::sleep(Duration::from_millis(
                super::super::mouse_pressure::RAMP_MS + 8,
            ));
            consume_mouse(r, slide, Gesture::Pressure, c.mouse_pressure)?;
            let coarse_level = MOUSE.with(|m| m.borrow().load.current().map(|(_, a)| a));
            let settle = |r: &mut LiveWheel, wanted: Option<f32>| -> Result<(), String> {
                let start = Instant::now();
                while MOUSE.with(|m| m.borrow().load.current().map(|(_, v)| v)) != wanted {
                    if start.elapsed() > Duration::from_millis(500) {
                        return Err(format!("{name} pressure did not reach {wanted:?}"));
                    }
                    std::thread::sleep(Duration::from_millis(4));
                    consume_mouse(r, slide, Gesture::Pressure, c.mouse_pressure)?;
                }
                Ok(())
            };
            let coarse_image = frozen(r)?;
            settle(r, Some(c.mouse_pressure))?;
            let pressed = frozen(r)?;
            let hold = frozen(r)?;
            r.select_track(c.layer_id);
            let isolated = frozen(r)?;
            r.select_track(0);
            scripted_mouse(hwnd, c.rect, WM_LBUTTONUP, point);
            consume_mouse(r, slide, Gesture::Pressure, c.mouse_pressure)?;
            std::thread::sleep(Duration::from_millis(
                super::super::mouse_pressure::RAMP_MS + 8,
            ));
            consume_mouse(r, slide, Gesture::Pressure, c.mouse_pressure)?;
            settle(r, None)?;
            let released = frozen(r)?;
            let points = rec.finish_timed()?;
            let mut target = c.clone();
            target.slide = slide;
            target.layer_id = 0;
            let mut player = macros::Playback::default();
            let mut reproduced = true;
            let mut peak_checked = false;
            for (_, s) in &points {
                player.apply(
                    Some(r),
                    Some(&target),
                    ReplayBatch {
                        layer_id: 0,
                        slide,
                        gesture: Gesture::Pressure,
                        tug_response: None,
                        samples: vec![*s],
                    },
                )?;
                if s.amount == Some(c.mouse_pressure) {
                    reproduced &= frozen(r)? == pressed;
                    peak_checked = true;
                }
                if matches!(s.phase, Phase::Up) {
                    reproduced &= frozen(r)? == neutral;
                }
            }
            let n = changed(&pressed, &neutral);
            let bytes_changed =
                |a: &[u8], b: &[u8]| a.iter().zip(b).filter(|(x, y)| x != y).count();
            let report = json!({"slide":slide,"case":name,"requestedCenter":point,"positiveControl":true,
                "afterCoarseWaitAmount":coarse_level,"coarseWaitVsSettledBytes":bytes_changed(&coarse_image,&pressed),
                "pressedPixelsAtLeast3Codes":n,"holdMismatchedBytes":bytes_changed(&pressed,&hold),
                "releaseMismatchedBytes":bytes_changed(&released,&neutral),"untouchedTrackMismatchedBytes":bytes_changed(&isolated,&neutral),
                "replayPeakChecked":peak_checked,"replayByteIdentical":reproduced,
                "recordedSamples":points.iter().map(|(t,s)|json!({"elapsedMs":t-points[0].0,"phase":s.phase,"center":s.point,"amount":s.amount})).collect::<Vec<_>>()});
            std::fs::write(
                self.path.with_extension(format!("pressure-{name}.json")),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            if n == 0
                || pressed != hold
                || isolated != neutral
                || released != neutral
                || !peak_checked
                || !reproduced
            {
                return Err(format!("{name} center={point:?} changedPixels={n}, holdBytes={}, isolatedBytes={}, releasedBytes={}, replayPeak={peak_checked}, replayEqual={reproduced}",bytes_changed(&pressed,&hold),bytes_changed(&isolated,&neutral),bytes_changed(&released,&neutral)));
            }
            let [w, h] = r.preview_size();
            let mut f =
                std::fs::File::create_new(self.path.with_extension(format!("pressure-{name}.ppm")))
                    .map_err(|e| e.to_string())?;
            write!(f, "P6\n{w} {h}\n255\n").map_err(|e| e.to_string())?;
            for row in pressed.chunks_exact(w as usize * 4) {
                f.write_all(
                    &row.chunks_exact(4)
                        .flat_map(|p| [p[0], p[1], p[2]])
                        .collect::<Vec<_>>(),
                )
                .map_err(|e| e.to_string())?;
            }
            reports.push(report);
        }
        r.select_track(c.layer_id);
        r.forget_track(0);
        self.proof = json!({"timeSeconds":0.65,"pressure":c.mouse_pressure,
            "slideResults":reports,"size":r.preview_size()});
        Ok(())
    }
    fn proof(&mut self, r: &mut LiveWheel, hwnd: HWND, c: &Config) -> Result<(), String> {
        // Editor layer IDs start at one. Keep the verification stroke and lens
        // changes off the live track and out of the visible presentation queue.
        const PROOF_TRACK: u64 = 0;
        r.select_track(PROOF_TRACK);
        r.render_offscreen(0.65)?;
        let (w, h, first) = r.capture_rgba8()?;
        r.render_offscreen(0.65)?;
        if r.capture_rgba8()?.2 != first {
            return Err("Repeated frozen state changed before the gesture".into());
        }
        let mut captures = vec![("before", first)];
        let mut changes = Vec::new();
        for (mode, name) in [(Gesture::Tug, "tug"), (Gesture::Rotate, "rotate")] {
            r.select_track(PROOF_TRACK);
            let recording = macros::ProofRecorder::start();
            scripted_mouse(hwnd, c.rect, WM_LBUTTONDOWN, [0.64, 0.5]);
            let mut point = [0.64, 0.5];
            for i in 1..=36 {
                let a = i as f64 * std::f64::consts::FRAC_PI_2 / 36.;
                point = [0.5 + 0.14 * a.cos(), 0.5 + 0.21 * a.sin()];
                scripted_mouse(hwnd, c.rect, WM_MOUSEMOVE, point);
                consume_mouse(r, c.slide, mode, c.mouse_pressure)?;
                r.render_offscreen(0.65)?;
            }
            scripted_mouse(hwnd, c.rect, WM_LBUTTONUP, point);
            consume_mouse(r, c.slide, mode, c.mouse_pressure)?;
            r.render_offscreen(0.65)?;
            let (_, _, after) = r.capture_rgba8()?;
            let points = recording.finish()?;
            let recorded_count = points.len();
            r.forget_track(PROOF_TRACK);
            let mut target = c.clone();
            target.layer_id = PROOF_TRACK;
            let mut replay = macros::Playback::default();
            // Split the recording to exercise cursor state across IPC batches.
            for batch in points.chunks(8) {
                replay.apply(
                    Some(r),
                    Some(&target),
                    ReplayBatch {
                        layer_id: PROOF_TRACK,
                        slide: c.slide,
                        gesture: mode,
                        tug_response: c.tug_response,
                        samples: batch.to_vec(),
                    },
                )?;
            }
            r.render_offscreen(0.65)?;
            if r.capture_rgba8()?.2 != after {
                return Err(format!("Recorded {name} replay changed the frozen result"));
            }
            let changed = captures[0]
                .1
                .chunks_exact(4)
                .zip(after.chunks_exact(4))
                .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) >= 3))
                .count();
            if changed == 0 {
                return Err(format!("{name} did not change the frozen live render"));
            }
            changes.push(json!({"gesture":mode,"changedPixelsAtLeast3Codes":changed,
                "recordedPoints":recorded_count,"replayChangedPixels":0}));
            captures.push((name, after));
            r.select_track(c.layer_id);
            r.forget_track(PROOF_TRACK);
        }
        r.select_track(PROOF_TRACK);
        for (name, focus) in [("focus-rear", 0.0), ("focus-front", 1.0)] {
            r.set_focus(Some(focus))?;
            r.render_offscreen(0.65)?;
            captures.push((name, r.capture_rgba8()?.2));
        }
        r.set_focus(c.focus)?;
        r.select_track(c.layer_id);
        r.forget_track(PROOF_TRACK);
        let focus_changed = captures[3]
            .1
            .chunks_exact(4)
            .zip(captures[4].1.chunks_exact(4))
            .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) >= 3))
            .count();
        if focus_changed == 0 {
            return Err("The two focal planes produced identical samples".into());
        }
        for (name, data) in captures {
            let mut file =
                std::fs::File::create_new(self.path.with_extension(format!("{name}.ppm")))
                    .map_err(|e| e.to_string())?;
            write!(file, "P6\n{w} {h}\n255\n").map_err(|e| e.to_string())?;
            let rgb: Vec<_> = data
                .chunks_exact(4)
                .flat_map(|v| v[..3].iter().copied())
                .collect();
            file.write_all(&rgb).map_err(|e| e.to_string())?;
        }
        self.proof = json!({"timeSeconds":0.65,"slide":c.slide,"presented":false,"isolatedTrack":true,
            "gestures":changes,"repeatControlChangedPixels":0,"focusEndpointsChangedPixelsAtLeast3Codes":focus_changed,"width":w,"height":h});
        Ok(())
    }
    pub fn before(&mut self, r: &mut LiveWheel, hwnd: HWND, c: &Config) -> Result<(), String> {
        let release = self.settings.motion != MotionPreset::WheelFlow;
        let pressure = self.settings.gesture == Gesture::Pressure;
        let (wheel, slide, gesture) = if release || pressure || self.settings.wheel_stack {
            (
                self.settings.wheel,
                self.settings.slide,
                self.settings.gesture,
            )
        } else {
            RUNS[self.index]
        };
        if c.wheel != wheel
            || c.slide != slide
            || c.gesture != gesture
            || !self.settings.matches_optics(c, r.preview_size())
        {
            if self.began.is_some() || Instant::now() > self.deadline {
                return Err(format!(
                    "Expected {wheel} {slide_name}/{gesture_name}, {motion:?}, focus {focus:?}, width {width}; got {actual_wheel} {actual_slide}/{actual_gesture}, {actual_motion:?}, focus {actual_focus:?}, width {actual_width}, rendered {preview:?}",
                    slide_name=json!(slide), gesture_name=json!(gesture), motion=self.settings.motion,
                    focus=self.settings.focus, width=self.settings.render_width, actual_wheel=c.wheel,
                    actual_slide=json!(c.slide), actual_gesture=json!(c.gesture), actual_motion=c.motion,
                    actual_focus=c.focus, actual_width=c.render_width, preview=r.preview_size()
                ));
            }
            return Ok(());
        }
        if self.began.is_none() {
            if self.index == 0 {
                if self.settings.wheel_stack {
                    let other = &c.stack[1];
                    let frozen = |r: &mut LiveWheel, time| -> Result<Vec<u8>, String> {
                        r.set_layer_times(other.layer_id, [time; 2])?;
                        r.render_offscreen(0.65)?;
                        Ok(r.capture_rgba8()?.2)
                    };
                    let a = frozen(r, 0.2)?;
                    let b = frozen(r, 2.)?;
                    let count = a
                        .chunks_exact(4)
                        .zip(b.chunks_exact(4))
                        .filter(|(u, v)| {
                            u[..3].iter().zip(&v[..3]).any(|(x, y)| x.abs_diff(*y) >= 3)
                        })
                        .count();
                    if count == 0 {
                        return Err(
                            "The second wheel did not move through the frozen first wheel".into(),
                        );
                    }
                    self.proof = json!({"selectedFieldTime":0.65,"secondFieldTimes":[0.2,2.0],"changedPixelsAtLeast3Codes":count});
                } else if release {
                    let beads = self.settings.motion == MotionPreset::WheelWithCoralBeads;
                    let frame = |r: &mut LiveWheel, time| -> Result<Vec<u8>, String> {
                        if beads {
                            r.render_film_times_offscreen(2., time, 0.)?;
                        } else {
                            r.render_offscreen(time)?;
                        }
                        Ok(r.capture_rgba8()?.2)
                    };
                    let first = frame(r, 0.)?;
                    let middle = frame(r, 1.9)?;
                    let last = frame(r, 4.)?;
                    let held = frame(r, 6.)?;
                    let changed = |a: &[u8], b: &[u8]| {
                        a.chunks_exact(4)
                            .zip(b.chunks_exact(4))
                            .filter(|(x, y)| (0..3).any(|i| x[i].abs_diff(y[i]) >= 3))
                            .count()
                    };
                    let early = changed(&first, &middle);
                    let late = changed(&middle, &last);
                    if early == 0 || late == 0 || held != last {
                        return Err(
                            "Coral release failed to advance or hold its final state".into()
                        );
                    }
                    self.proof = json!({"sampleTimes":[0.,1.9,4.,6.],
                        "earlyChangedPixelsAtLeast3Codes":early, "lateChangedPixelsAtLeast3Codes":late,
                        "heldByteIdentical":true});
                    if beads {
                        let replay = frame(r, 0.)?;
                        r.render_film_times_offscreen(3.5, 6., 0.)?;
                        let moving_rear = changed(&held, &r.capture_rgba8()?.2);
                        if replay != first || moving_rear == 0 {
                            return Err("Front beads failed to replay at fixed rear phase or the rear wheel stopped with the front held".into());
                        }
                        self.proof["fixedRearSeconds"] = json!(2.);
                        self.proof["frontReplayByteIdenticalToStart"] = json!(true);
                        self.proof["rearTimesWithFrontHeldAt6Seconds"] = json!([2., 3.5]);
                        self.proof["rearChangedPixelsAtLeast3Codes"] = json!(moving_rear);
                    }
                    self.initial_take = c.motion_take;
                } else if pressure {
                    self.pressure_proof(r, hwnd, c)?;
                } else {
                    self.proof(r, hwnd, c)?;
                }
            }
            if self.index < 2 && !release {
                let suffix = if pressure {
                    "pressure.frames"
                } else if self.index == 0 {
                    "tug.frames"
                } else {
                    "rotate.frames"
                };
                self.motion = Some(capture::Capture::new(
                    self.path.with_extension(suffix),
                    Duration::from_secs(3),
                )?);
                self.motion_drags = 0;
            }
            self.stroke_started = Instant::now();
            self.began = Some(self.stroke_started);
            if !pressure {
                scripted_mouse(hwnd, c.rect, WM_LBUTTONDOWN, [0.64, 0.5]);
            }
        }
        let phase = self.stroke_started.elapsed().as_secs_f64() * 2.35;
        if pressure {
            let held = self.stroke_started.elapsed().as_secs_f64() % 2.0 < 1.3;
            let center = [0.48 + 0.055 * phase.sin(), 0.52 + 0.04 * phase.cos()];
            let message = match (self.pressing, held) {
                (false, true) => WM_LBUTTONDOWN,
                (true, true) => WM_MOUSEMOVE,
                (true, false) => WM_LBUTTONUP,
                (false, false) => return Ok(()),
            };
            self.pressing = held;
            scripted_mouse(hwnd, c.rect, message, center);
            return Ok(());
        }
        let point = [0.50 + 0.14 * phase.cos(), 0.50 + 0.21 * phase.sin()];
        if self.settings.motion == MotionPreset::WheelWithCoralBeads {
            if let Some(replay) = self.replays.last() {
                if c.motion_take != replay.motion_take {
                    return Ok(()); // The pointer left the slide to click Replay front beads.
                }
            }
            if MOUSE.with(|m| m.borrow().previous.is_none()) {
                scripted_mouse(hwnd, c.rect, WM_LBUTTONDOWN, point);
                if let Some(replay) = self.replays.last_mut() {
                    replay.stroke_starts += 1;
                }
                return Ok(());
            }
        }
        scripted_mouse(hwnd, c.rect, WM_MOUSEMOVE, point);
        Ok(())
    }

    pub fn after(
        &mut self,
        did_present: bool,
        drags: usize,
        frame_time: Duration,
        r: &LiveWheel,
        hwnd: HWND,
        c: &Config,
        app: &tauri::AppHandle,
    ) -> Result<bool, String> {
        let Some(start) = self.began else {
            return Ok(false);
        };
        let now = Instant::now();
        if let Some(record) = self.motion.as_mut() {
            self.motion_drags += drags as u64;
            if did_present && record.frame(r, c, frame_time, self.motion_drags)? {
                self.motion = None;
                self.began = Some(now);
            }
            return Ok(false);
        }
        let elapsed = now.duration_since(start);
        if elapsed < WARM {
            return Ok(false);
        }
        if self.measured_from_unix_ms.is_none() {
            self.measured_from_unix_ms =
                Some(unix_ms()?.saturating_sub((elapsed - WARM).as_millis() as u64));
        }
        let release = self.settings.motion != MotionPreset::WheelFlow;
        let beads = self.settings.motion == MotionPreset::WheelWithCoralBeads;
        if release {
            let elapsed_ms = (elapsed - WARM).as_secs_f64() * 1000.;
            if let Some(replay) = self.replays.last_mut() {
                if c.motion_take == replay.motion_take {
                    replay.mouse_moves_applied += drags as u64;
                    if replay.first_presented_ms.is_none() && did_present {
                        replay.first_presented_ms = Some(elapsed_ms);
                    }
                }
            }
            let count = self.replays.len() as u64;
            if count < 3
                && (elapsed - WARM).as_secs() >= count * 4
                && c.motion_take == self.initial_take + count
                && self
                    .replays
                    .last()
                    .is_none_or(|p| p.first_presented_ms.is_some())
            {
                if beads {
                    if let Some(point) = MOUSE.with(|m| m.borrow().previous) {
                        scripted_mouse(hwnd, c.rect, WM_LBUTTONUP, point.map(f64::from));
                    }
                }
                app.emit("tss-benchmark-release", ())
                    .map_err(|e| e.to_string())?;
                self.replays.push(ReplayObservation {
                    motion_take: c.motion_take + 1,
                    requested_ms: elapsed_ms,
                    first_presented_ms: None,
                    stroke_starts: 0,
                    mouse_moves_applied: 0,
                });
            }
        }
        if did_present {
            self.presents += 1;
            if let Some(prev) = self.last.replace(now) {
                self.intervals
                    .push(now.duration_since(prev).as_secs_f64() * 1000.);
            }
        } else {
            self.skips += 1;
        }
        self.drags += drags as u64;
        if elapsed < WARM + PERIOD {
            return Ok(false);
        }
        let failure = if self.intervals.is_empty() {
            Some("Fewer than two successful surface presentations")
        } else if self.drags == 0 {
            Some("No native mouse moves applied during the measurement")
        } else if release
            && (self.replays.len() != 3
                || c.motion_take != self.initial_take + 3
                || self.replays.iter().any(|p| p.first_presented_ms.is_none()))
        {
            Some("The editor did not present all three coral replays")
        } else if beads
            && self
                .replays
                .iter()
                .any(|p| p.stroke_starts == 0 || p.mouse_moves_applied == 0)
        {
            Some("A replayed front-bead take lacked a native press and drag")
        } else {
            None
        };
        if let Some(reason) = failure {
            return Err(format!(
                "{reason}: {}",
                json!({
                    "elapsedMs":(elapsed-WARM).as_millis() as u64, "presents":self.presents,
                    "skips":self.skips,"intervals":self.intervals.len(),"mouseMovesApplied":self.drags,
                    "initialTake":self.initial_take,"currentTake":c.motion_take,"replays":self.replays,
                    "preview":r.preview_size(),"previewHidden":c.preview_hidden
                })
            ));
        }
        scripted_mouse(hwnd, c.rect, WM_LBUTTONUP, [0.5, 0.5]);
        let n = self.intervals.len() as f64;
        let seconds = self.intervals.iter().sum::<f64>() / 1000.;
        self.results.push(json!({"wheel":c.wheel,"stack":c.stack,"motion":c.motion,"replayRequests":self.replays.len(),
            "replays":self.replays,"measuredFromUnixMs":self.measured_from_unix_ms,"measuredThroughUnixMs":unix_ms()?,
            "slide":c.slide,"gesture":c.gesture,"tugResponse":c.tug_response,"presented":self.presents,
            "skipped":self.skips,"mouseMovesApplied":self.drags,"secondsBetweenPresents":seconds,
            "fps": n/seconds,"p50Ms":percentile(&mut self.intervals,0.5),
            "p95Ms":percentile(&mut self.intervals,0.95),"p99Ms":percentile(&mut self.intervals,0.99),
            "focus":c.focus,"surface":[c.rect.width,c.rect.height],"preview":r.preview_size()}));
        self.index += 1;
        if self.index == self.count {
            let out = json!({"adapter":r.adapter_name(),"field":r.field_size(),"maxFrameLatency":2,
                "clock":"std::time::Instant after successful FIFO surface present returns",
                "footprintSampling":self.footprint_sampling,
                "input":"window-local scripted WM_LBUTTONDOWN/WM_MOUSEMOVE/WM_LBUTTONUP",
                "frozenGesture":self.proof,"wheels":self.results});
            let mut file = std::fs::File::create_new(&self.path).map_err(|e| e.to_string())?;
            serde_json::to_writer_pretty(&mut file, &out).map_err(|e| e.to_string())?;
            return Ok(true);
        }
        let (wheel, slide, gesture) = RUNS[self.index];
        app.emit(
            "tss-benchmark-wheel",
            json!({"wheel":wheel,"slide":slide,"gesture":gesture}),
        )
        .map_err(|e| e.to_string())?;
        self.deadline = now + Duration::from_secs(25);
        self.began = None;
        self.measured_from_unix_ms = None;
        self.last = None;
        self.intervals.clear();
        self.presents = 0;
        self.skips = 0;
        self.drags = 0;
        Ok(false)
    }
}
