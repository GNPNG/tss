use serde::{Deserialize, Serialize};
use tauri::{State, WebviewWindow};
#[cfg(any(windows, test))]
mod mouse_pressure;
pub mod movie;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Host;
#[cfg(not(windows))]
#[derive(Default)]
pub struct Host;

#[derive(Clone, Copy, PartialEq, Deserialize)]
pub struct Rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Clone, Deserialize, Serialize, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Slide {
    Rear,
    Front,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Gesture {
    #[default]
    Tug,
    Rotate,
    Pressure,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct GesturePoint {
    phase: Phase,
    point: [f32; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<f32>,
    #[serde(default, rename = "ageMs", skip_serializing)]
    age_ms: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayBatch {
    layer_id: u64,
    slide: Slide,
    gesture: Gesture,
    tug_response: Option<f32>,
    samples: Vec<GesturePoint>,
}

impl ReplayBatch {
    fn validate(&self) -> Result<(), String> {
        if self.layer_id == 0
            || self.samples.is_empty()
            || self.samples.len() > 512
            || self.tug_response.is_some_and(|n| !(0.0..=1.0).contains(&n))
            || self.samples.iter().any(|s| {
                s.point.iter().any(|n| !(0.0..=1.0).contains(n))
                    || s.age_ms.is_some_and(|n| !(0.0..=60_000.0).contains(&n))
                    || !valid_amount(self.gesture, s.phase, s.amount)
            })
        {
            return Err("Invalid performance track, sample, pressure or coordinates".into());
        }
        Ok(())
    }
}

fn valid_amount(gesture: Gesture, phase: Phase, amount: Option<f32>) -> bool {
    match (gesture, phase, amount) {
        (Gesture::Pressure, Phase::Up, None | Some(0.0)) => true,
        (Gesture::Pressure, Phase::Down | Phase::Move, Some(v)) => (0.0..=1.0).contains(&v),
        (Gesture::Tug | Gesture::Rotate, _, None) => true,
        _ => false,
    }
}

#[tauri::command]
pub async fn perform_live_wheel_gesture(
    host: State<'_, Host>,
    batch: ReplayBatch,
) -> Result<(), String> {
    batch.validate()?;
    #[cfg(windows)]
    return host.perform(batch).await;
    #[cfg(not(windows))]
    {
        let _ = host;
        Err("Native slide performance requires Windows.".into())
    }
}

#[tauri::command]
pub async fn set_live_wheel_pressure(
    host: State<'_, Host>,
    layer_id: u64,
    slide: Slide,
    amount: f32,
    center: [f32; 2],
    phase: Phase,
    age_ms: Option<f64>,
) -> Result<(), String> {
    perform_live_wheel_gesture(
        host,
        ReplayBatch {
            layer_id,
            slide,
            gesture: Gesture::Pressure,
            tug_response: None,
            samples: vec![GesturePoint {
                phase,
                point: center,
                amount: Some(amount),
                age_ms,
            }],
        },
    )
    .await
}

#[tauri::command]
pub async fn set_live_wheel_capture_enabled(
    host: State<'_, Host>,
    enabled: bool,
) -> Result<f64, String> {
    #[cfg(windows)]
    return host.record(enabled).await;
    #[cfg(not(windows))]
    {
        let _ = (host, enabled);
        Err("Native gesture capture requires Windows.".into())
    }
}

#[tauri::command]
pub async fn replay_live_wheel_gesture(
    host: State<'_, Host>,
    batch: ReplayBatch,
) -> Result<(), String> {
    batch.validate()?;
    #[cfg(windows)]
    return host.replay(batch).await;
    #[cfg(not(windows))]
    {
        let _ = (host, batch);
        Err("Native gesture playback requires Windows.".into())
    }
}

#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackConfig {
    delay_ms: u32,
    strength: f32,
    zoom: f32,
    rotation_degrees: f32,
}
#[cfg(windows)]
impl FeedbackConfig {
    fn native(&self) -> tss_live_wheel::FeedbackSettings {
        tss_live_wheel::FeedbackSettings {
            delay_ms: self.delay_ms,
            strength: self.strength,
            zoom: self.zoom,
            rotation_degrees: self.rotation_degrees,
        }
    }
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackAnchor {
    epoch: u64,
    playing: bool,
    position: f64,
    at_unix_ms: f64,
    #[serde(default)]
    ended: bool,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    layer_id: u64,
    wheel: String,
    slide: Slide,
    #[serde(default)]
    gesture: Gesture,
    tug_response: Option<f32>,
    #[serde(default = "default_pressure")]
    mouse_pressure: f32,
    render_width: u32,
    rect: Rect,
    focus: Option<f32>,
    #[serde(default)]
    feedback: Option<FeedbackConfig>,
    #[serde(default)]
    feedback_clock: Option<FeedbackAnchor>,
    #[serde(default)]
    preview_hidden: bool,
}
fn default_pressure() -> f32 {
    0.5
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    available: bool,
    running: bool,
    adapter: String,
    width: u32,
    height: u32,
    fps: f64,
    p95_ms: f64,
    presents: u64,
    drags: u64,
    frame_latency: u32,
    layer_id: u64,
    slide: Option<Slide>,
    gesture: Option<Gesture>,
    focus: Option<f32>,
    error: Option<String>,
    projector_running: bool,
    projector_age_ms: Option<f64>,
    projector_song_time: Option<f64>,
    projector_presents: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VisibilityProof {
    hidden_ms: f64,
    offscreen_frames: u64,
    both_hidden_ms: f64,
    both_hidden_frames: u64,
}
#[tauri::command]
pub async fn verify_projector_visibility(host: State<'_, Host>) -> Result<VisibilityProof, String> {
    #[cfg(windows)]
    return host.verify_visibility().await;
    #[cfg(not(windows))]
    {
        let _ = host;
        Err("Projector visibility verification requires Windows.".into())
    }
}

#[tauri::command]
pub async fn set_projector_output(
    host: State<'_, Host>,
    enabled: bool,
    delay_ms: u32,
) -> Result<(), String> {
    #[cfg(windows)]
    return host.projector(enabled, delay_ms).await;
    #[cfg(not(windows))]
    {
        let _ = (host, delay_ms);
        if enabled {
            Err("Native projector output requires Windows.".into())
        } else {
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn set_live_wheel(
    window: WebviewWindow,
    host: State<'_, Host>,
    config: Option<Config>,
) -> Result<(), String> {
    #[cfg(windows)]
    return host.set(window, config);
    #[cfg(not(windows))]
    {
        let _ = (window, host);
        if config.is_some() {
            Err("Native wheel preview requires Windows.".into())
        } else {
            Ok(())
        }
    }
}

#[tauri::command]
pub fn forget_live_wheel_layer(host: State<'_, Host>, layer_id: u64) -> Result<(), String> {
    #[cfg(windows)]
    return host.forget(layer_id);
    #[cfg(not(windows))]
    {
        let _ = (host, layer_id);
        Ok(())
    }
}

#[tauri::command]
pub fn live_wheel_stats(host: State<'_, Host>) -> Stats {
    #[cfg(windows)]
    return host.stats();
    #[cfg(not(windows))]
    {
        let _ = host;
        Stats::default()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkConfig {
    wheel: &'static str,
    slide: Slide,
    gesture: Gesture,
    render_width: u32,
    verify_export: bool,
}

#[tauri::command]
pub fn live_wheel_benchmark_config() -> Option<BenchmarkConfig> {
    #[cfg(windows)]
    if windows::benchmark_output().is_some() || crate::export_smoke::enabled() {
        return Some(BenchmarkConfig {
            wheel: "violet-blue",
            slide: Slide::Rear,
            gesture: if windows::pressure_trial() {
                Gesture::Pressure
            } else {
                Gesture::Tug
            },
            render_width: 640,
            verify_export: crate::export_smoke::enabled(),
        });
    }
    None
}
