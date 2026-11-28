use super::{Gesture, Host, Phase, Slide};
use crate::{selected_export_paths, SelectedPaths};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;

pub const FPS: u32 = 30;
pub const SIZE: [u32; 2] = [3840, 2160];

#[derive(Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Baseline {
    Current,
    Neutral,
}

impl Baseline {
    #[cfg(any(windows, target_os = "macos", test))]
    pub(super) fn film_origins(self, last_presented: [f64; 2]) -> [f64; 2] {
        match self {
            Self::Current => last_presented,
            Self::Neutral => [0.; 2],
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub time: f64,
    pub layer_id: u64,
    pub slide: Slide,
    pub gesture: Gesture,
    pub tug_response: Option<f32>,
    pub phase: Phase,
    pub point: [f32; 2],
    pub amount: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(flatten)]
    pub(super) lighting: super::Lighting,
    pub(super) focus: Option<f32>,
    #[serde(default)]
    pub(super) detail_scale: Option<f32>,
    #[serde(default)]
    pub(super) stack: Vec<super::StackLayer>,
    #[serde(default)]
    pub(super) visible_films: super::VisibleFilms,
    pub layer_id: u64,
    pub wheel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) swatches: Option<[u32; 4]>,
    #[serde(default)]
    pub(super) motion: super::MotionPreset,
    #[serde(default)]
    pub(super) material_motion: super::MaterialMotion,
    #[serde(default)]
    pub(super) motion_take: u64,
    pub duration: f64,
    pub baseline: Baseline,
    pub events: Vec<Event>,
}
impl Request {
    pub fn validate(&self) -> Result<u64, String> {
        if self.layer_id == 0
            || self.detail_scale.is_some_and(|s| !(0.4..=2.0).contains(&s))
            || self.wheel.is_empty()
            || !self.duration.is_finite()
            || self.duration <= 0.
            || self.duration * FPS as f64 >= (1u64 << 53) as f64
        {
            return Err("Choose a wheel and a finite song duration".into());
        }
        let mut ids = std::collections::HashSet::new();
        for layer in &self.stack {
            if layer.layer_id == 0
                || !ids.insert(layer.layer_id)
                || !(0.0..=1.0).contains(&layer.opacity)
                || (layer.media.is_none() && layer.wheel.is_empty())
                || layer.media.as_ref().is_some_and(|m| !m.valid())
            {
                return Err("Invalid wheel stack in export".into());
            }
        }
        if self.stack.is_empty() {
            ids.insert(self.layer_id);
        }
        let mut held = std::collections::HashMap::new();
        let mut previous = 0.;
        for event in &self.events {
            if !ids.contains(&event.layer_id)
                || !(previous..=self.duration).contains(&event.time)
                || event.point.iter().any(|v| !(0.0..=1.0).contains(v))
                || !super::valid_amount(event.gesture, event.phase, event.amount)
                || event
                    .tug_response
                    .is_some_and(|v| !(0.0..=1.0).contains(&v))
            {
                return Err("Invalid wheel export event timing, track or coordinates".into());
            }
            let anchor = held
                .entry((
                    event.layer_id,
                    event.slide as u8,
                    event.gesture == Gesture::Pressure,
                ))
                .or_insert(None);
            match event.phase {
                Phase::Down => *anchor = Some(event.gesture),
                Phase::Move if *anchor != Some(event.gesture) => {
                    return Err("Export gesture move requires a matching down".into())
                }
                Phase::Up => *anchor = None,
                _ => (),
            }
            previous = event.time;
        }
        Ok((self.duration * FPS as f64).ceil() as u64)
    }
}

#[derive(Default)]
pub struct ExportControl(std::sync::Mutex<Option<Arc<AtomicBool>>>);

#[tauri::command]
pub async fn export_native_wheel(
    paths: State<'_, SelectedPaths>,
    host: State<'_, Host>,
    control: State<'_, ExportControl>,
    audio_path: String,
    output_path: String,
    request: Request,
) -> Result<(), String> {
    request.validate()?;
    let paths = selected_export_paths(&paths, &audio_path, &output_path)?;
    #[cfg(any(windows, target_os = "macos"))]
    {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut busy = control.0.lock().map_err(|e| e.to_string())?;
            if busy.is_some() {
                return Err("A native wheel export is already in progress".into());
            }
            *busy = Some(cancel.clone());
        }
        let result = host.export_movie(paths, request, cancel).await;
        *control.0.lock().map_err(|e| e.to_string())? = None;
        result
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (control, host, paths);
        Err("Native wheel export requires the Windows or macOS GPU editor".into())
    }
}

#[tauri::command]
pub async fn cancel_native_wheel_export(control: State<'_, ExportControl>) -> Result<(), String> {
    if let Some(cancel) = control.0.lock().map_err(|e| e.to_string())?.as_ref() {
        cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_wheel_requests_and_explicit_release_takes_decode_independently() {
        let mut body = serde_json::json!({"layerId":2,"wheel":"violet-blue",
            "duration":2.0,"baseline":"neutral","events":[]});
        let parse = |value| serde_json::from_value::<Request>(value);
        let legacy = parse(body.clone()).unwrap();
        assert_eq!(legacy.motion, super::super::MotionPreset::WheelFlow);
        assert_eq!(legacy.motion_take, 0);
        assert_eq!(
            legacy.material_motion,
            super::super::MaterialMotion::default()
        );
        body["materialMotion"] = serde_json::json!({"rear":"pdms-1000"});
        body["stack"] = serde_json::json!([{"layerId":2,"wheel":"violet-blue","opacity":1.0,
            "materialMotion":{"front":"pdms-20"}}]);
        let authored = parse(body.clone()).unwrap();
        let json = |m: super::super::MaterialMotion| serde_json::to_value(m).unwrap();
        assert_eq!(
            json(authored.material_motion),
            serde_json::json!({"rear":"pdms-1000","front":"wheel"})
        );
        assert_eq!(
            json(authored.stack[0].material_motion),
            serde_json::json!({"rear":"wheel","front":"pdms-20"})
        );
        let mut invalid = body.clone();
        invalid["materialMotion"]["rear"] = serde_json::json!("pdms-200");
        assert!(parse(invalid).is_err());
        assert_eq!(legacy.visible_films.slides(), [true, true]);
        for (mode, shown) in [
            ("rear", [true, false]),
            ("front", [false, true]),
            ("clear", [false, false]),
        ] {
            body["visibleFilms"] = serde_json::json!(mode);
            assert_eq!(parse(body.clone()).unwrap().visible_films.slides(), shown);
        }
        body["motion"] = serde_json::json!("coral-release");
        body["motionTake"] = serde_json::json!(4);
        let coral = parse(body.clone()).unwrap();
        assert_eq!(coral.motion, super::super::MotionPreset::CoralRelease);
        assert_eq!(coral.motion_take, 4);
        body["motion"] = serde_json::json!("wheel-with-coral-beads");
        let mixed = parse(body.clone()).unwrap();
        assert_eq!(
            mixed.motion,
            super::super::MotionPreset::WheelWithCoralBeads
        );
        assert_eq!(mixed.motion_take, 4);
        body["motion"] = serde_json::json!("unsupported");
        assert!(parse(body).is_err());
    }
    #[test]
    fn current_export_preserves_both_source_positions_and_neutral_restarts_both() {
        let at_replay = [12.5, 0.25];
        assert_eq!(Baseline::Current.film_origins(at_replay), [12.5, 0.25]);
        assert_eq!(Baseline::Neutral.film_origins(at_replay), [0.; 2]);
    }
    fn event(time: f64, slide: Slide, phase: Phase) -> Event {
        Event {
            time,
            layer_id: 2,
            slide,
            gesture: Gesture::Rotate,
            tug_response: Some(0.4),
            phase,
            point: [0.4, 0.5],
            amount: None,
        }
    }
    fn request(events: Vec<Event>) -> Request {
        Request {
            lighting: serde_json::from_str("{}").unwrap(),
            focus: None,
            detail_scale: None,
            stack: Vec::new(),
            visible_films: super::super::VisibleFilms::Both,
            layer_id: 2,
            wheel: "violet-blue".into(),
            swatches: None,
            motion: Default::default(),
            material_motion: Default::default(),
            motion_take: 0,
            duration: 0.75,
            baseline: Baseline::Neutral,
            events,
        }
    }
    #[test]
    fn native_movie_checks_each_slides_anchors_and_event_time_before_rendering() {
        assert_eq!(
            request(vec![
                event(0., Slide::Rear, Phase::Down),
                event(0., Slide::Front, Phase::Down),
                event(0.20, Slide::Front, Phase::Move),
                event(0.30, Slide::Rear, Phase::Move),
                event(0.7, Slide::Rear, Phase::Up)
            ])
            .validate()
            .unwrap(),
            23
        );
        assert!(request(vec![
            event(0., Slide::Rear, Phase::Down),
            event(0.2, Slide::Front, Phase::Move)
        ])
        .validate()
        .is_err());
        assert!(request(vec![
            event(0.3, Slide::Rear, Phase::Down),
            event(0.1, Slide::Rear, Phase::Move)
        ])
        .validate()
        .is_err());
        let mut wrong = event(0., Slide::Front, Phase::Down);
        wrong.layer_id = 1;
        assert!(request(vec![wrong]).validate().is_err());
    }

    #[test]
    fn pressure_and_planar_motion_on_one_slide_have_independent_contacts() {
        let force = |t, phase, amount| Event {
            gesture: Gesture::Pressure,
            amount,
            ..event(t, Slide::Front, phase)
        };
        let mixed = vec![
            event(0., Slide::Front, Phase::Down),
            force(0.1, Phase::Down, Some(0.3)),
            event(0.2, Slide::Front, Phase::Move),
            force(0.3, Phase::Move, Some(0.62)),
            force(0.4, Phase::Up, Some(0.)),
            event(0.5, Slide::Front, Phase::Move),
        ];
        assert_eq!(request(mixed).validate().unwrap(), 23);
        for invalid in [
            force(0., Phase::Move, Some(0.4)),
            force(0., Phase::Down, Some(1.1)),
            force(0., Phase::Down, None),
            force(0., Phase::Up, Some(0.8)),
        ] {
            assert!(request(vec![invalid]).validate().is_err());
        }
    }
}
