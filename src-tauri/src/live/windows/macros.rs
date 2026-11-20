// Capture only native input. Replayed points never pass through the recorder.
use super::*;
use serde::Serialize;
use std::cell::Cell;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;

thread_local! { static POSTED_TIME: Cell<Option<u32>> = const { Cell::new(None) }; }
pub(super) fn posted_time(time: Option<u32>) {
    POSTED_TIME.set(time);
}
pub(super) fn now_ms() -> u64 {
    unsafe { GetTickCount64() }
}
pub(super) fn event_ms() -> u64 {
    let now = now_ms();
    // MSG.time is the OS event's posting time, low 32 bits of the uptime
    // clock. Preserve the spacing of moves that are delivered in one pump.
    POSTED_TIME.get().map_or(now, |posted| {
        now.saturating_sub(u64::from((now as u32).wrapping_sub(posted)))
    })
}

pub(super) type Reply<T> = tauri::async_runtime::Sender<Result<T, String>>;

impl Host {
    pub(super) async fn request<T>(
        &self,
        make: impl FnOnce(Reply<T>) -> Command,
    ) -> Result<T, String> {
        let (reply, mut response) = tauri::async_runtime::channel(1);
        self.tx
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .ok_or("Select a native wheel first")?
            .send(make(reply))
            .map_err(|_| "Native wheel stopped")?;
        response.recv().await.ok_or("Native wheel stopped")?
    }

    pub async fn record(&self, enabled: bool) -> Result<f64, String> {
        self.request(|reply| Command::Record(enabled, reply)).await
    }

    pub async fn replay(&self, batch: ReplayBatch) -> Result<(), String> {
        self.request(|reply| Command::Replay(batch, reply)).await
    }
    pub async fn perform(&self, batch: ReplayBatch) -> Result<(), String> {
        let received = now_ms();
        self.request(|reply| Command::Perform(batch, received, reply))
            .await
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimedPoint {
    #[serde(flatten)]
    sample: GesturePoint,
    time_ms: f64,
}

#[derive(Default)]
pub(super) struct Recorder {
    last_ms: u64,
    enabled: bool,
    samples: Vec<TimedPoint>,
}

impl Recorder {
    pub fn push_at(&mut self, t: u64, sample: GesturePoint) {
        if self.enabled {
            self.last_ms = self.last_ms.max(t);
            self.samples.push(TimedPoint {
                sample,
                time_ms: self.last_ms as f64,
            });
        }
    }
    pub fn push(&mut self, phase: Phase, point: [f32; 2]) {
        if self.enabled {
            self.last_ms = self.last_ms.max(event_ms());
            self.samples.push(TimedPoint {
                sample: GesturePoint {
                    phase,
                    point,
                    amount: None,
                    age_ms: None,
                },
                time_ms: self.last_ms as f64,
            });
        }
    }
}

// The benchmark's disposable track uses the same input recorder and replay
// state machine as the controls. Keep proof input out of the operator's take.
pub(super) struct ProofRecorder(Option<Recorder>);
impl ProofRecorder {
    pub fn start() -> Self {
        let isolated = Recorder {
            enabled: true,
            ..Default::default()
        };
        Self(Some(MOUSE.with(|m| {
            std::mem::replace(&mut m.borrow_mut().recorder, isolated)
        })))
    }
    pub fn finish(self) -> Result<Vec<GesturePoint>, String> {
        Ok(self.finish_timed()?.into_iter().map(|(_, s)| s).collect())
    }
    pub fn finish_timed(mut self) -> Result<Vec<(f64, GesturePoint)>, String> {
        let isolated =
            MOUSE.with(|m| std::mem::replace(&mut m.borrow_mut().recorder, self.0.take().unwrap()));
        if !matches!(
            isolated.samples.first().map(|p| p.sample.phase),
            Some(Phase::Down)
        ) || !matches!(
            isolated.samples.last().map(|p| p.sample.phase),
            Some(Phase::Up)
        ) || isolated
            .samples
            .windows(2)
            .any(|pair| pair[0].time_ms > pair[1].time_ms)
        {
            return Err("Native input recording lost event order".into());
        }
        Ok(isolated
            .samples
            .into_iter()
            .map(|p| (p.time_ms, p.sample))
            .collect())
    }
}
impl Drop for ProofRecorder {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            MOUSE.with(|m| m.borrow_mut().recorder = previous);
        }
    }
}

pub(super) fn record(
    app: &tauri::AppHandle,
    config: Option<&Config>,
    enabled: bool,
) -> Result<f64, String> {
    if enabled && config.is_none() {
        return Err("Select a native wheel before recording".into());
    }
    let time_ms = MOUSE.with(|m| {
        let mut m = m.borrow_mut();
        let now = now_ms();
        m.recorder.last_ms = now;
        if enabled != m.recorder.enabled {
            let previous = m.previous;
            if m.pressure_mode {
                let contact = m.load.current();
                if enabled {
                    m.recorder.enabled = true;
                }
                if let Some((point, amount)) = contact {
                    m.recorder.push_at(
                        now,
                        GesturePoint {
                            phase: if enabled { Phase::Down } else { Phase::Up },
                            point,
                            amount: Some(if enabled { amount } else { 0. }),
                            age_ms: None,
                        },
                    );
                }
                m.recorder.enabled = enabled;
                return now as f64;
            }
            if !enabled {
                if let Some(p) = previous {
                    m.recorder.push(Phase::Up, p);
                }
            }
            m.recorder.enabled = enabled;
            if enabled {
                if let Some(p) = previous {
                    m.recorder.push(Phase::Down, p);
                }
            }
        }
        now as f64
    });
    if let Some(c) = config {
        flush(app, c)?;
    }
    Ok(time_ms)
}

pub(super) fn flush(app: &tauri::AppHandle, c: &Config) -> Result<(), String> {
    MOUSE.with(|m| {
        let mut m = m.borrow_mut();
        if !m.recorder.samples.is_empty() {
            app.emit(
                "tss-live-gesture",
                json!({
                    "layerId":c.layer_id,"slide":c.slide,"gesture":c.gesture,
                    "tugResponse":c.tug_response.unwrap_or(0.4), "samples":m.recorder.samples,
                }),
            )
            .map_err(|e| e.to_string())?;
            m.recorder.samples.clear();
        }
        Ok(())
    })
}

#[derive(Default)]
pub(super) struct Playback {
    held: [Option<(Gesture, [f32; 2])>; 2],
    pressure: [Option<([f32; 2], f32)>; 2],
    response: [f32; 2],
}

impl Playback {
    pub fn physical_press(&mut self, slide: Slide, gesture: Gesture) {
        if gesture == Gesture::Pressure {
            self.pressure[slide as usize] = None;
        } else {
            self.held[slide as usize] = None;
        }
    }

    pub fn release(&mut self, r: &mut LiveWheel) -> Result<(), String> {
        for (slide, held) in self.held.iter_mut().enumerate() {
            if held.take().is_some() {
                r.end_gesture(slide as u32);
            }
        }
        for (slide, held) in self.pressure.iter_mut().enumerate() {
            if let Some((point, _)) = held.take() {
                r.set_slide_pressure(slide as u32, 0., point)?;
            }
        }
        Ok(())
    }

    pub fn occupied(&self, slide: Slide, gesture: Gesture) -> bool {
        let i = slide as usize;
        if gesture == Gesture::Pressure {
            self.pressure[i].is_some()
        } else {
            self.held[i].is_some()
        }
    }

    pub fn signal(
        &self,
        app: &tauri::AppHandle,
        layer: u64,
        phase: Phase,
        time: f64,
    ) -> Result<(), String> {
        for slide in [Slide::Rear, Slide::Front] {
            let i = slide as usize;
            let xy = self.held[i].map(|(g, p)| (g, p, None));
            let press = self.pressure[i].map(|(p, a)| {
                (
                    Gesture::Pressure,
                    p,
                    Some(if matches!(phase, Phase::Up) { 0. } else { a }),
                )
            });
            for (gesture, point, amount) in [xy, press].into_iter().flatten() {
                let sample = TimedPoint {
                    sample: GesturePoint {
                        phase,
                        point,
                        amount,
                        age_ms: None,
                    },
                    time_ms: time,
                };
                app.emit(
                    "tss-live-gesture",
                    json!({"layerId":layer,"slide":slide,"gesture":gesture,
                    "tugResponse":self.response[i],"samples":[sample]}),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn apply(
        &mut self,
        engine: Option<&mut LiveWheel>,
        config: Option<&Config>,
        b: ReplayBatch,
    ) -> Result<(), String> {
        let c = config
            .filter(|c| c.layer_id == b.layer_id)
            .ok_or("Macro target must be the active native preview track")?;
        let r = engine.ok_or("Native wheel is unavailable")?;
        self.apply_to(r, c, &b, true)
    }

    fn apply_to(
        &mut self,
        r: &mut LiveWheel,
        c: &Config,
        b: &ReplayBatch,
        selected: bool,
    ) -> Result<(), String> {
        let i = b.slide as usize;
        let pressure = b.gesture == Gesture::Pressure;
        // Validate the complete batch before any operation can change the pose.
        let mut active = if pressure {
            self.pressure[i].map(|_| Gesture::Pressure)
        } else {
            self.held[i].map(|(gesture, _)| gesture)
        };
        for point in &b.samples {
            match point.phase {
                Phase::Down => active = Some(b.gesture),
                Phase::Move if active != Some(b.gesture) => {
                    return Err("Replay move requires a matching down event".into())
                }
                Phase::Up => active = None,
                _ => (),
            }
        }
        if selected
            && b.slide == c.slide
            && pressure == (c.gesture == Gesture::Pressure)
            && MOUSE.with(|m| {
                let m = m.borrow();
                m.previous.is_some() || pressure && m.load.current().is_some()
            })
        {
            // A cancel during a physical drag may discard our replay anchor,
            // but must not release the operator's ongoing physical gesture.
            if b.samples.iter().all(|s| matches!(s.phase, Phase::Up)) {
                self.physical_press(b.slide, b.gesture);
                return Ok(());
            }
            return Err("Release the mouse on this slide before playing its macro".into());
        }
        if pressure {
            for s in &b.samples {
                if matches!(s.phase, Phase::Up) && self.pressure[i].is_none() {
                    continue;
                }
                r.set_slide_pressure(i as u32, s.amount.unwrap_or(0.), s.point)?;
                self.pressure[i] = if matches!(s.phase, Phase::Up) {
                    None
                } else {
                    Some((s.point, s.amount.unwrap_or(0.)))
                };
            }
            return Ok(());
        }
        let mode = native_mode(b.gesture).ok_or("Expected a planar gesture")?;
        self.response[i] = b.tug_response.unwrap_or(0.4);
        r.set_tug_response(i as u32, b.tug_response.unwrap_or(0.4))?;
        for sample in &b.samples {
            match sample.phase {
                Phase::Down => {
                    r.end_gesture(i as u32);
                    self.held[i] = Some((b.gesture, sample.point));
                }
                Phase::Move => {
                    let (_, from) = self.held[i].unwrap();
                    r.gesture(i as u32, mode, from, sample.point);
                    self.held[i] = Some((b.gesture, sample.point));
                }
                Phase::Up => {
                    if self.held[i].take().is_some() {
                        r.end_gesture(i as u32);
                    }
                }
            }
        }
        if selected && b.slide == c.slide {
            r.set_tug_response(i as u32, c.tug_response.unwrap_or(0.4))?;
        }
        Ok(())
    }
}
pub(super) fn recording() -> bool {
    MOUSE.with(|m| m.borrow().recorder.enabled)
}

pub(super) fn perform(
    performers: &mut std::collections::BTreeMap<u64, Playback>,
    r: &mut LiveWheel,
    c: &Config,
    batch: ReplayBatch,
    received: u64,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let id = batch.layer_id;
    let selected = id == c.layer_id;
    if !selected {
        r.select_track(id);
    }
    let result = performers
        .entry(id)
        .or_default()
        .apply_to(r, c, &batch, selected);
    if !selected {
        r.select_track(c.layer_id);
    }
    result?;
    if MOUSE.with(|m| m.borrow().recorder.enabled) {
        let samples: Vec<_> = batch
            .samples
            .iter()
            .map(|sample| TimedPoint {
                sample: *sample,
                time_ms: (received as f64 - sample.age_ms.unwrap_or(0.)).max(0.),
            })
            .collect();
        app.emit(
            "tss-live-gesture",
            json!({"layerId":id,"slide":batch.slide,
            "gesture":batch.gesture,"tugResponse":batch.tug_response,"samples":samples}),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
