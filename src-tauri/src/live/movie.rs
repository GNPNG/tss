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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub layer_id: u64,
    pub wheel: String,
    pub duration: f64,
    pub baseline: Baseline,
    pub events: Vec<Event>,
}
impl Request {
    pub fn validate(&self) -> Result<u64, String> {
        if self.layer_id == 0
            || self.wheel.is_empty()
            || !self.duration.is_finite()
            || self.duration <= 0.
            || self.duration * FPS as f64 >= (1u64 << 53) as f64
        {
            return Err("Choose a wheel and a finite song duration".into());
        }
        let mut held = [None; 2];
        let mut previous = 0.;
        for event in &self.events {
            if event.layer_id != self.layer_id
                || !(previous..=self.duration).contains(&event.time)
                || event.point.iter().any(|v| !(0.0..=1.0).contains(v))
                || event
                    .tug_response
                    .is_some_and(|v| !(0.0..=1.0).contains(&v))
            {
                return Err("Invalid wheel export event timing, track or coordinates".into());
            }
            let anchor = &mut held[event.slide as usize];
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
    #[cfg(windows)]
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
    #[cfg(not(windows))]
    {
        let _ = (control, host, paths);
        Err("Native wheel export currently requires the Windows GPU editor".into())
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
    fn event(time: f64, slide: Slide, phase: Phase) -> Event {
        Event {
            time,
            layer_id: 2,
            slide,
            gesture: Gesture::Rotate,
            tug_response: Some(0.4),
            phase,
            point: [0.4, 0.5],
        }
    }
    fn request(events: Vec<Event>) -> Request {
        Request {
            layer_id: 2,
            wheel: "violet-blue".into(),
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
}
