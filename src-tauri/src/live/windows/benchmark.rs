// Interactive benchmark uses the editor's existing wheel picker and child window.
// A local HWND stroke exercises normal input handling without moving the OS pointer.
use super::*;
use std::io::Write;

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

pub(super) struct Benchmark {
    path: PathBuf,
    footprint_sampling: bool,
    index: usize,
    count: usize,
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
        Self {
            path,
            footprint_sampling,
            index: 0,
            count: if std::env::args_os().any(|arg| arg == "--gesture-pair") {
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
                consume_mouse(r, c.slide, mode);
                r.render_offscreen(0.65)?;
            }
            scripted_mouse(hwnd, c.rect, WM_LBUTTONUP, point);
            consume_mouse(r, c.slide, mode);
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
        let (wheel, slide, gesture) = RUNS[self.index];
        if c.wheel != wheel || c.slide != slide || c.gesture != gesture {
            if Instant::now() > self.deadline {
                return Err(format!(
                    "Editor did not select the requested controls for {wheel}"
                ));
            }
            return Ok(());
        }
        if self.began.is_none() {
            if self.index == 0 {
                self.proof(r, hwnd, c)?;
            }
            if self.index < 2 {
                let suffix = if self.index == 0 {
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
            scripted_mouse(hwnd, c.rect, WM_LBUTTONDOWN, [0.64, 0.5]);
        }
        let phase = self.stroke_started.elapsed().as_secs_f64() * 2.35;
        scripted_mouse(
            hwnd,
            c.rect,
            WM_MOUSEMOVE,
            [0.50 + 0.14 * phase.cos(), 0.50 + 0.21 * phase.sin()],
        );
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
        if self.intervals.is_empty() || self.drags == 0 {
            return Err("Insufficient interactive frame samples".into());
        }
        scripted_mouse(hwnd, c.rect, WM_LBUTTONUP, [0.5, 0.5]);
        let n = self.intervals.len() as f64;
        let seconds = self.intervals.iter().sum::<f64>() / 1000.;
        self.results.push(json!({"wheel":c.wheel,"slide":c.slide,"gesture":c.gesture,"tugResponse":c.tug_response,"presented":self.presents,
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
        self.last = None;
        self.intervals.clear();
        self.presents = 0;
        self.skips = 0;
        self.drags = 0;
        Ok(false)
    }
}
