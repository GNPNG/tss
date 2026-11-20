use super::MotionPreset;
use std::collections::HashMap;
use std::time::Duration;

/// Global wheel flow keeps its continuous phase. Each coral take begins on
/// that layer's first frame and retains its origin across focus and track changes.
#[derive(Default)]
pub(super) struct ReleaseClock {
    starts: HashMap<u64, (u64, Duration)>,
}
impl ReleaseClock {
    pub fn field_time(
        &mut self,
        layer: u64,
        motion: MotionPreset,
        take: u64,
        now: Duration,
    ) -> f64 {
        if motion == MotionPreset::WheelFlow {
            return now.as_secs_f64();
        }
        let entry = self.starts.entry(layer).or_insert((take, now));
        if take != entry.0 {
            *entry = (take, now);
        }
        now.saturating_sub(entry.1).as_secs_f64()
    }
    pub fn forget(&mut self, layer: u64) {
        self.starts.remove(&layer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_selection_and_replay_start_from_zero_on_the_selected_layer_only() {
        use MotionPreset::*;
        let mut c = ReleaseClock::default();
        let mut time = |layer, motion, take, second| {
            c.field_time(layer, motion, take, Duration::from_secs(second))
        };
        assert_eq!(time(1, WheelFlow, 0, 20), 20.);
        assert_eq!(time(1, CoralRelease, 1, 60), 0.);
        assert_eq!(time(2, CoralRelease, 1, 61), 0.);
        assert_eq!(time(1, CoralRelease, 1, 62), 2.);
        assert_eq!(time(1, CoralRelease, 2, 63), 0.);
        assert_eq!(time(2, CoralRelease, 1, 64), 3.);
        assert_eq!(time(1, WheelFlow, 2, 65), 65.);
        c.forget(2);
        assert_eq!(
            c.field_time(2, CoralRelease, 1, Duration::from_secs(66)),
            0.
        );
    }
}
