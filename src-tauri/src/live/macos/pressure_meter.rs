use crate::live::{Config, Gesture, Slide, Stats};
use serde::Serialize;
use tauri::{Emitter, WebviewWindow};

#[derive(Clone, Copy)]
pub(super) struct Contact {
    pub(super) point: [f32; 2],
    pub(super) raw: f32,
    pub(super) applied: f32,
}

impl Contact {
    pub(super) fn new(point: [f32; 2], raw: f32, sensitivity: f32) -> Self {
        // The power curve keeps both endpoints fixed across the full control range.
        let applied = raw.powf(sensitivity.recip());
        Self {
            point,
            raw,
            applied,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Reading {
    layer_id: u64,
    slide: Slide,
    raw: f32,
    applied: f32,
}

impl Reading {
    fn new(layer_id: u64, slide: Slide, contact: Option<Contact>) -> Self {
        Self {
            layer_id,
            slide,
            raw: contact.map_or(0., |c| c.raw),
            applied: contact.map_or(0., |c| c.applied),
        }
    }
}

#[derive(Default)]
pub(super) struct PressureMeter {
    last: Option<Reading>,
}

impl PressureMeter {
    // Sample at presentation cadence. Contact release is also sent immediately
    // because hiding the preview can suspend its frame clock.
    pub(super) fn publish(
        &mut self,
        window: &WebviewWindow,
        stats: &mut Stats,
        c: &Config,
        contact: Option<Contact>,
    ) {
        if c.gesture != Gesture::Pressure {
            return;
        }
        let reading = Reading::new(c.layer_id, c.slide, contact);
        stats.trackpad_pressure = reading.raw;
        stats.trackpad_pressure_applied = reading.applied;
        if self.last != Some(reading) {
            self.last = Some(reading);
            let _ = window.emit("tss-trackpad-pressure", reading);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitivity_preserves_full_range_and_orders_intermediate_forces() {
        for sensitivity in [0.25, 0.5, 1., 2., 4.] {
            let curve = |raw| Contact::new([0.5; 2], raw, sensitivity).applied;
            assert_eq!(curve(0.), 0.);
            assert_eq!(curve(1.), 1.);
            for i in 0..100 {
                let raw = i as f32 / 100.;
                assert!(curve(raw) < curve((i + 1) as f32 / 100.));
            }
        }
        let response = |s| Contact::new([0.5; 2], 0.25, s).applied;
        assert_eq!(response(0.5), 0.0625);
        assert_eq!(response(1.), 0.25);
        assert_eq!(response(2.), 0.5);
    }

    #[test]
    fn meter_reports_renderer_load_and_distinguishes_release_and_track_changes() {
        let contact = Contact::new([0.5; 2], 0.25, 2.);
        let pressed = Reading::new(7, Slide::Rear, Some(contact));
        let sample = serde_json::to_value(pressed).unwrap();
        assert_eq!(
            sample,
            serde_json::json!({"layerId":7, "slide":"rear", "raw":0.25, "applied":0.5})
        );
        let released = Reading::new(7, Slide::Rear, None);
        assert_eq!((released.raw, released.applied), (0., 0.));
        assert!(pressed != released);
        assert!(pressed != Reading::new(8, Slide::Rear, Some(contact)));
        assert!(pressed != Reading::new(7, Slide::Front, Some(contact)));
    }
}
