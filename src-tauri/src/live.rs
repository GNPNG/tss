use serde::{Deserialize, Serialize};
use tauri::{State, WebviewWindow};
#[cfg(any(windows, target_os = "macos"))]
use tss_live_wheel::media_track::MediaSettings;
#[cfg(not(any(windows, target_os = "macos")))]
#[path = "../../live-wheel/src/media_track.rs"]
mod media_track;
#[cfg(not(any(windows, target_os = "macos")))]
use media_track::MediaSettings;
#[cfg(any(windows, target_os = "macos"))]
use tss_live_wheel::{MaterialMotion, MotionPreset, OpacityEnvelope};
#[cfg(not(any(windows, target_os = "macos")))]
#[path = "../../live-wheel/src/envelope.rs"]
mod envelope;
#[cfg(not(any(windows, target_os = "macos")))]
use envelope::OpacityEnvelope;
#[cfg(not(any(windows, target_os = "macos")))]
#[path = "../../live-wheel/src/preset.rs"]
mod preset;
#[cfg(not(any(windows, target_os = "macos")))]
use preset::{MaterialMotion, MotionPreset};
#[cfg(any(windows, target_os = "macos", test))]
mod mouse_pressure;
pub mod movie;
#[cfg(any(windows, target_os = "macos"))]
mod movie_render;
pub mod plates;
#[cfg(any(windows, target_os = "macos", test))]
mod release_clock;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Host;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::watch_verification;
#[cfg(target_os = "macos")]
pub use macos::Host;
#[cfg(not(any(windows, target_os = "macos")))]
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

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum VisibleFilms {
    #[default]
    Both,
    Rear,
    Front,
    Clear,
}
impl VisibleFilms {
    #[cfg(any(windows, target_os = "macos", test))]
    fn slides(self) -> [bool; 2] {
        [
            matches!(self, Self::Both | Self::Rear),
            matches!(self, Self::Both | Self::Front),
        ]
    }
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
#[cfg(any(windows, target_os = "macos"))]
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
#[derive(Default, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum ProjectorId { #[default] A, B }
fn primary(id: &ProjectorId) -> bool { *id == ProjectorId::A }
fn neutral_kelvin() -> u32 { 6500 }
fn full_light() -> f32 { 1.0 }

#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecondProjector {
    enabled: bool,
    lamp_kelvin: u32,
    focus: Option<f32>,
    intensity: f32,
    offset_x: f32,
    offset_y: f32,
    equipment: Option<Equipment>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct Equipment { profile: EquipmentProfile }
#[derive(Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum EquipmentProfile {
    Custom, LiquidOverhead, Film, #[serde(rename = "carousel-550r")] Carousel550r
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Lighting {
    #[serde(default = "neutral_kelvin")]
    lamp_kelvin: u32,
    #[serde(default = "full_light")]
    projector_a_intensity: f32,
    equipment_a: Option<Equipment>,
    projector_b: Option<SecondProjector>,
}

#[derive(Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(flatten)]
    lighting: Lighting,
    #[serde(default)]
    detail_scale: Option<f32>,
    #[serde(default)]
    simulation: Option<Simulation>,
    #[serde(default)]
    stack: Vec<StackLayer>,
    #[serde(default)]
    visible_films: VisibleFilms,
    layer_id: u64,
    wheel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    swatches: Option<[u32; 4]>,
    #[serde(default)]
    motion: MotionPreset,
    #[serde(default)]
    material_motion: MaterialMotion,
    #[serde(default)]
    motion_take: u64,
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
#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackLayer {
    #[serde(default, skip_serializing_if = "primary")]
    projector: ProjectorId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    detail_scale: Option<f32>,
    layer_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    simulation: Option<Simulation>,
    #[serde(default)]
    media: Option<MediaSettings>,
    #[serde(default)]
    matte_only: bool,
    #[serde(default)]
    wheel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    swatches: Option<[u32; 4]>,
    #[serde(default)]
    motion: MotionPreset,
    #[serde(default)]
    material_motion: MaterialMotion,
    #[serde(default)]
    motion_take: u64,
    #[serde(default)]
    visible_films: VisibleFilms,
    opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    envelope: Option<OpacityEnvelope>,
}

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Simulation {
    seed: u32,
    rate: f64,
    /// A located film has one dyed cell. Older projects without a depth
    /// retain the original paired-cell initial conditions and trajectories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    depth: Option<f32>,
}

// Locate the sidecar in the signed application bundle or portable app directory.
// The webview cannot provide a process name or executable path.
fn solver_executable() -> Option<std::path::PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let name = if cfg!(windows) { "tss-slide-solver.exe" } else { "tss-slide-solver" };
    let path = executable.parent()?.join(name);
    (cfg!(feature = "live-solver") && path.is_file()).then_some(path)
}

#[tauri::command]
pub fn live_simulation_available() -> bool {
    solver_executable().is_some()
}

impl Config {
    fn uses_simulation(&self) -> bool {
        self.simulation.is_some() || self.stack.iter().any(|track| track.simulation.is_some())
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn read_solver_status(&self, stats: &mut Stats, mut status: impl FnMut(u64) -> Option<tss_live_wheel::LiveSolverStatus>) {
        stats.simulation_tick = None;
        let mut read = |id| {
            if let Some(worker) = status(id) {
                if id == self.layer_id && worker.state == "ready" {
                    stats.simulation_tick = Some(worker.tick);
                }
                if let Some(error) = worker.error {
                    // Preserve an existing media decode error when a mixed program loses both sources.
                    let message = format!("Fluid solver on track {id}: {error}");
                    match &mut stats.error {
                        Some(previous) => { previous.push_str(". "); previous.push_str(&message); }
                        None => stats.error = Some(message),
                    }
                }
            }
        };
        if self.stack.is_empty() {
            if self.simulation.is_some() { read(self.layer_id); }
        } else {
            for track in &self.stack {
                if track.simulation.is_some() { read(track.layer_id); }
            }
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn configure_simulation(&self, renderer: &mut tss_live_wheel::LiveWheel) -> Result<(), String> {
        let lighting = &self.lighting;
        renderer.set_lamp_temperature(lighting.lamp_kelvin)?;
        renderer.set_primary_intensity(lighting.projector_a_intensity)?;
        renderer.set_projector_routing(
            lighting.projector_b.as_ref().map(|b| tss_live_wheel::BeamSettings {
                enabled: b.enabled, focus: b.focus, lamp_kelvin: b.lamp_kelvin,
                intensity: b.intensity, offset: [b.offset_x, b.offset_y],
            }),
            self.stack.iter().filter(|t| t.projector == ProjectorId::B).map(|t| t.layer_id).collect(),
        )?;
        renderer.set_projector_lenses(
            lighting.equipment_a.as_ref().is_some_and(|e| e.profile == EquipmentProfile::Carousel550r),
            lighting.projector_b.as_ref().and_then(|b| b.equipment.as_ref()).is_some_and(|e| e.profile == EquipmentProfile::Carousel550r),
        );
        for track in &self.stack {
            if track.layer_id != self.layer_id && track.media.is_none() {
                renderer.set_detail_scale(track.layer_id, track.detail_scale.unwrap_or(1.0))?;
            }
        }
        renderer.set_detail_scale(self.layer_id, self.detail_scale.unwrap_or(1.0))?;
        #[cfg(feature = "live-solver")]
        {
            let mut apply = |id, sim: Option<Simulation>| -> Result<(), String> {
                renderer.set_film_depth(id, sim.and_then(|s| s.depth))?;
                if let Some(sim) = sim {
                    if !(0.0..=2.0).contains(&sim.rate) {
                        return Err("Simulation speed must be between 0 and 2".into());
                    }
                    let exe = solver_executable().ok_or("The live fluid solver is missing from this app")?;
                    if sim.depth.is_some() {
                        renderer.set_single_cell_solver(id, &exe, sim.seed, sim.rate)?;
                    } else {
                        renderer.set_live_solver(id, &exe, sim.seed, sim.rate)?;
                    }
                } else {
                    renderer.clear_live_solver(id);
                }
                Ok(())
            };
            for track in &self.stack {
                if track.layer_id != self.layer_id && track.media.is_none() {
                    apply(track.layer_id, track.simulation)?;
                }
            }
            apply(self.layer_id, self.simulation)
        }
        #[cfg(not(feature = "live-solver"))]
        {
            let _ = renderer;
            if self.uses_simulation() {
                return Err("This build lacks the live fluid solver required by this track".into());
            }
            Ok(())
        }
    }
}

fn default_pressure() -> f32 {
    0.5
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    #[serde(skip_serializing_if = "Option::is_none")]
    simulation_tick: Option<u64>,
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
    #[cfg(target_os = "macos")]
    return host.set(window, config).await;
    #[cfg(not(any(windows, target_os = "macos")))]
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
    #[cfg(any(windows, target_os = "macos"))]
    return host.forget(layer_id);
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (host, layer_id);
        Ok(())
    }
}

#[tauri::command]
pub fn live_wheel_stats(host: State<'_, Host>) -> Stats {
    #[cfg(any(windows, target_os = "macos"))]
    return host.stats();
    #[cfg(not(any(windows, target_os = "macos")))]
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
    wheel_stack: bool,
    media_trial: bool,
    verify_plates: bool,
    coral_trial: bool,
    motion: MotionPreset,
    focus: Option<f32>,
}

#[cfg(any(windows, target_os = "macos", test))]
impl BenchmarkConfig {
    fn from_flags(
        pressure: bool,
        coral: bool,
        beads: bool,
        preview_960: bool,
        verify_export: bool,
    ) -> Self {
        let motion = if beads {
            MotionPreset::WheelWithCoralBeads
        } else if coral {
            MotionPreset::CoralRelease
        } else {
            MotionPreset::WheelFlow
        };
        Self {
            wheel: "violet-blue",
            slide: if coral || beads {
                Slide::Front
            } else {
                Slide::Rear
            },
            gesture: if pressure && !coral && !beads {
                Gesture::Pressure
            } else {
                Gesture::Tug
            },
            render_width: if preview_960 { 960 } else { 640 },
            verify_export,
            wheel_stack: false,
            media_trial: false,
            verify_plates: false,
            coral_trial: motion == MotionPreset::CoralRelease,
            motion,
            focus: beads.then_some(0.90),
        }
    }

    #[cfg(windows)]
    fn current() -> Self {
        let flag = |name: &str| std::env::args_os().any(|arg| arg == name);
        let mut config = Self::from_flags(
            windows::pressure_trial(),
            flag("--coral-trial"),
            flag("--coral-beads-trial"),
            flag("--preview-960"),
            crate::export_smoke::enabled(),
        );
        config.wheel_stack = flag("--wheel-stack-trial");
        config.media_trial = flag("--media-trial") && windows::benchmark_output().is_some();
        config.verify_plates = windows::plate_verification().is_some();
        config
    }

    #[cfg(any(windows, test))]
    fn matches_optics(&self, c: &Config, preview: [u32; 2]) -> bool {
        (!self.wheel_stack
            || c.stack.len() == 2
                && c.stack[0].wheel == "violet-blue"
                && c.stack[1].wheel == "yellow-red")
            && (!self.media_trial
                || (c.stack.len() == 3
                    && c.stack[0].layer_id == c.layer_id
                    && c.stack[0].wheel == "violet-blue"
                    && c.stack[1].media.is_some()
                    && c.stack[1].matte_only
                    && c.stack[2]
                        .media
                        .as_ref()
                        .is_some_and(|m| m.key_enabled && m.mask_a == Some(c.stack[1].layer_id))))
            && c.motion == self.motion
            && c.focus == self.focus
            && c.render_width == self.render_width
            && preview == [self.render_width, self.render_width * 9 / 16]
    }
}

#[tauri::command]
pub fn live_wheel_benchmark_config() -> Option<BenchmarkConfig> {
    #[cfg(windows)]
    if windows::benchmark_output().is_some()
        || crate::export_smoke::enabled()
        || windows::plate_verification().is_some()
    {
        return Some(BenchmarkConfig::current());
    }
    #[cfg(target_os = "macos")]
    if crate::export_smoke::enabled() {
        let flag = |name: &str| std::env::args_os().any(|arg| arg == name);
        let mut config = BenchmarkConfig::from_flags(
            flag("--verify-pressure-export"),
            flag("--coral-trial"),
            flag("--coral-beads-trial"),
            flag("--preview-960"),
            true,
        );
        config.wheel_stack = flag("--wheel-stack-trial");
        return Some(config);
    }
    None
}

#[cfg(test)]
mod benchmark_tests {
    use super::*;
    use serde_json::json;

    #[test]
    #[cfg(any(windows, target_os = "macos"))]
    fn solver_status_reports_failed_background_matte_and_selected_tick() {
        let mut config: Config = serde_json::from_value(json!({
            "layerId":1,"wheel":"violet-blue","slide":"rear","gesture":"tug","renderWidth":640,
            "rect":{"x":0,"y":0,"width":640,"height":360},
            "simulation":{"seed":17,"rate":1},
            "stack":[{"layerId":1,"opacity":1,"simulation":{"seed":17,"rate":1}},
                     {"layerId":2,"opacity":1,"matteOnly":true,"simulation":{"seed":85,"rate":1}}]
        })).unwrap();
        let mut stats = Stats { error: Some("Missing artwork".into()), ..Default::default() };
        config.read_solver_status(&mut stats, |id| Some(tss_live_wheel::LiveSolverStatus {
            state: if id == 1 { "ready" } else { "failed" }, tick: 47, flow_rate: 1.,
            error: (id == 2).then(|| "Worker exited".into()),
        }));
        assert_eq!(stats.simulation_tick, Some(47));
        assert_eq!(stats.error.as_deref(), Some("Missing artwork. Fluid solver on track 2: Worker exited"));
        config.stack.clear();
        stats.error = None;
        config.read_solver_status(&mut stats, |_| Some(tss_live_wheel::LiveSolverStatus {
            state: "starting", tick: 0, flow_rate: 1., error: None,
        }));
        assert!(stats.simulation_tick.is_none() && stats.error.is_none());
    }

    #[test]
    fn benchmark_selects_the_named_motion_and_actual_preview() {
        for (pressure, coral, beads, motion, slide, gesture, focus) in [
            (false, false, false, "wheel-flow", "rear", "tug", None),
            (true, false, false, "wheel-flow", "rear", "pressure", None),
            (false, true, false, "coral-release", "front", "tug", None),
            (
                false,
                false,
                true,
                "wheel-with-coral-beads",
                "front",
                "tug",
                Some(0.90_f32),
            ),
        ] {
            for width in [640, 960] {
                let settings =
                    BenchmarkConfig::from_flags(pressure, coral, beads, width == 960, false);
                let ui = json!(settings);
                assert_eq!(ui["motion"], motion);
                assert_eq!(ui["slide"], slide);
                assert_eq!(ui["gesture"], gesture);
                assert_eq!(ui["focus"], json!(focus));
                assert_eq!(ui["renderWidth"], width);
                let mut c: Config = serde_json::from_value(json!({
                    "layerId":1, "wheel":"violet-blue", "motion":motion, "slide":slide, "gesture":gesture,
                    "renderWidth":width, "focus":focus, "rect":{"x":0,"y":0,"width":1200,"height":680}
                })).unwrap();
                let size = [width, width * 9 / 16];
                assert!(settings.matches_optics(&c, size));
                assert!(!settings.matches_optics(&c, [width - 16, size[1] - 9]));
                c.render_width = width - 16;
                assert!(!settings.matches_optics(&c, size));
                c.render_width = width;
                c.focus = Some(0.5);
                assert!(!settings.matches_optics(&c, size));
                c.focus = focus;
                c.motion = if coral {
                    MotionPreset::WheelWithCoralBeads
                } else {
                    MotionPreset::CoralRelease
                };
                assert!(!settings.matches_optics(&c, size));
            }
        }
    }
}
