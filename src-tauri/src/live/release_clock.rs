use super::MotionPreset;
use std::collections::HashMap;
use std::time::Duration;

/// Global wheel flow keeps its continuous phase. Each coral take begins on
/// that layer's first frame and retains its origin across focus and track changes.
#[derive(Default)]
pub(super) struct ReleaseClock {
    starts: HashMap<u64, (MotionPreset, u64, Duration)>,
}
impl ReleaseClock {
    /// Rear and front source positions for one presented frame.
    pub fn film_times(
        &mut self,
        layer: u64,
        motion: MotionPreset,
        take: u64,
        now: Duration,
    ) -> [f64; 2] {
        let global = now.as_secs_f64();
        if motion == MotionPreset::WheelFlow {
            return [global; 2];
        }
        let entry = self.starts.entry(layer).or_insert((motion, take, now));
        if motion != entry.0 || take != entry.1 {
            *entry = (motion, take, now);
        }
        let local = now.saturating_sub(entry.2).as_secs_f64();
        if motion == MotionPreset::WheelWithCoralBeads {
            [global, local]
        } else {
            [local; 2]
        }
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
            c.film_times(layer, motion, take, Duration::from_secs(second))
        };
        assert_eq!(time(1, WheelFlow, 0, 20), [20.; 2]);
        assert_eq!(time(1, CoralRelease, 1, 60), [0.; 2]);
        assert_eq!(time(2, CoralRelease, 1, 61), [0.; 2]);
        assert_eq!(time(1, CoralRelease, 1, 62), [2.; 2]);
        assert_eq!(time(1, CoralRelease, 2, 63), [0.; 2]);
        assert_eq!(time(2, CoralRelease, 1, 64), [3.; 2]);
        assert_eq!(time(1, WheelFlow, 2, 65), [65.; 2]);
        c.forget(2);
        assert_eq!(
            c.film_times(2, CoralRelease, 1, Duration::from_secs(66)),
            [0.; 2]
        );
    }

    #[test]
    fn selecting_and_replaying_front_beads_preserves_global_rear_motion() {
        use MotionPreset::*;
        let mut clock = ReleaseClock::default();
        let mut time = |layer, motion, take, second| {
            clock.film_times(layer, motion, take, Duration::from_secs(second))
        };
        assert_eq!(time(1, WheelFlow, 0, 15), [15., 15.]);
        assert_eq!(time(1, WheelWithCoralBeads, 1, 20), [20., 0.]);
        assert_eq!(time(2, WheelWithCoralBeads, 1, 22), [22., 0.]);
        assert_eq!(time(1, WheelWithCoralBeads, 1, 25), [25., 5.]);
        assert_eq!(time(1, WheelWithCoralBeads, 2, 27), [27., 0.]);
        assert_eq!(time(2, WheelWithCoralBeads, 1, 30), [30., 8.]);
        assert_eq!(time(1, CoralRelease, 2, 31), [0.; 2]);
    }
}
