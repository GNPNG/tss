// Capture only native input. Replayed points never pass through the recorder.
use super::*;
use serde::Serialize;
use std::cell::Cell;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;

thread_local! { static POSTED_TIME: Cell<Option<u32>> = const { Cell::new(None) }; }
pub(super) fn posted_time(time: Option<u32>) {
    POSTED_TIME.set(time);
}
fn now_ms() -> u64 {
    unsafe { GetTickCount64() }
}
fn event_ms() -> u64 {
    let now = now_ms();
    // MSG.time is the OS event's posting time, low 32 bits of the uptime
    // clock. Preserve the spacing of moves that are delivered in one pump.
    POSTED_TIME.get().map_or(now, |posted| {
        now.saturating_sub(u64::from((now as u32).wrapping_sub(posted)))
    })
}

pub(super) type Reply<T> = tauri::async_runtime::Sender<Result<T, String>>;

impl Host {
    pub(super) async fn request<T>(&self, make: impl FnOnce(Reply<T>) -> Command) -> Result<T, String> {
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
        if batch.layer_id == 0
            || batch.samples.is_empty()
            || batch.samples.len() > 512
            || batch
                .tug_response
                .is_some_and(|v| !(0.0..=1.0).contains(&v))
            || batch
                .samples
                .iter()
                .any(|s| s.point.iter().any(|v| !(0.0..=1.0).contains(v)))
        {
            return Err("Invalid replay track, sample count, response or coordinates".into());
        }
        self.request(|reply| Command::Replay(batch, reply)).await
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
    pub fn push(&mut self, phase: Phase, point: [f32; 2]) {
        if self.enabled {
            self.last_ms = self.last_ms.max(event_ms());
            self.samples.push(TimedPoint {
                sample: GesturePoint { phase, point },
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
    pub fn finish(mut self) -> Result<Vec<GesturePoint>, String> {
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
        Ok(isolated.samples.into_iter().map(|p| p.sample).collect())
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
}

impl Playback {
    pub fn physical_press(&mut self, slide: Slide) {
        self.held[slide as usize] = None;
    }

    pub fn release(&mut self, r: &mut LiveWheel) {
        for (slide, held) in self.held.iter_mut().enumerate() {
            if held.take().is_some() {
                r.end_gesture(slide as u32);
            }
        }
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
        let i = b.slide as usize;
        // Validate the complete batch before any operation can change the pose.
        let mut active = self.held[i].map(|(gesture, _)| gesture);
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
        if b.slide == c.slide && MOUSE.with(|m| m.borrow().previous.is_some()) {
            // A cancel during a physical drag may discard our replay anchor,
            // but must not release the operator's ongoing physical gesture.
            if b.samples.iter().all(|s| matches!(s.phase, Phase::Up)) {
                self.held[i] = None;
                return Ok(());
            }
            return Err("Release the mouse on this slide before playing its macro".into());
        }
        r.set_tug_response(i as u32, b.tug_response.unwrap_or(0.4))?;
        for sample in b.samples {
            match sample.phase {
                Phase::Down => {
                    r.end_gesture(i as u32);
                    self.held[i] = Some((b.gesture, sample.point));
                }
                Phase::Move => {
                    let (_, from) = self.held[i].unwrap();
                    r.gesture(i as u32, native_mode(b.gesture), from, sample.point);
                    self.held[i] = Some((b.gesture, sample.point));
                }
                Phase::Up => {
                    if self.held[i].take().is_some() {
                        r.end_gesture(i as u32);
                    }
                }
            }
        }
        if b.slide == c.slide {
            r.set_tug_response(i as u32, c.tug_response.unwrap_or(0.4))?;
        }
        Ok(())
    }
}
