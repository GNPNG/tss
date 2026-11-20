// A binary mouse button supplies an envelope. Each fixed-time force sample
// is also a native macro point, so offline frames sample the same performance.
use super::{GesturePoint, Phase};

pub(super) const RAMP_MS: u64 = 128;
const STEP_MS: u64 = 8;

#[derive(Clone, Copy)]
struct Ramp {
    start: u64,
    next: u64,
    from: f32,
    to: f32,
    releasing: bool,
}

#[derive(Default)]
pub(super) struct MousePressure {
    contact: Option<[f32; 2]>,
    amount: f32,
    ramp: Option<Ramp>,
    clock: u64,
}

impl MousePressure {
    pub fn current(&self) -> Option<([f32; 2], f32)> {
        self.contact.map(|p| (p, self.amount))
    }
    fn sample(&self, phase: Phase) -> GesturePoint {
        GesturePoint {
            phase,
            point: self.contact.unwrap(),
            amount: Some(self.amount),
            age_ms: None,
        }
    }
    pub fn advance(&mut self, now: u64, out: &mut Vec<(u64, GesturePoint)>) {
        self.clock = self.clock.max(now);
        while let Some(mut r) = self.ramp {
            if r.next > self.clock {
                break;
            }
            let t = (r.next - r.start) as f32 / RAMP_MS as f32;
            let eased = t * t * (3. - 2. * t);
            self.amount = r.from + (r.to - r.from) * eased;
            let finished = r.next == r.start + RAMP_MS;
            if finished {
                self.amount = r.to;
            }
            let release = finished && r.releasing;
            out.push((
                r.next,
                self.sample(if release { Phase::Up } else { Phase::Move }),
            ));
            if release {
                self.contact = None;
            }
            r.next += STEP_MS;
            self.ramp = (!finished).then_some(r);
        }
    }
    pub fn input(
        &mut self,
        phase: Phase,
        p: [f32; 2],
        level: f32,
        now: u64,
        out: &mut Vec<(u64, GesturePoint)>,
    ) {
        self.advance(now, out);
        if self.contact.is_none() && !matches!(phase, Phase::Down) {
            return;
        }
        let begin = self.contact.is_none();
        let moved = self.contact != Some(p);
        self.contact = Some(p);
        if begin || moved {
            out.push((
                self.clock,
                self.sample(if begin { Phase::Down } else { Phase::Move }),
            ));
        }
        if matches!(phase, Phase::Down | Phase::Up) {
            let releasing = matches!(phase, Phase::Up);
            self.ramp = Some(Ramp {
                start: self.clock,
                next: self.clock + STEP_MS,
                from: self.amount,
                to: if releasing { 0. } else { level },
                releasing,
            });
        }
    }
    pub fn detach(&mut self, now: u64) -> Option<(u64, GesturePoint)> {
        self.contact?;
        self.amount = 0.;
        let sample = self.sample(Phase::Up);
        self.ramp = None;
        self.contact = None;
        Some((now.max(self.clock), sample))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stationary_click_has_deterministic_attack_hold_and_release_at_any_poll_rate() {
        let render = |times: &[u64]| {
            let mut pressure = MousePressure::default();
            let mut out = vec![];
            pressure.input(Phase::Down, [0.44, 0.38], 0.8, 1000, &mut out);
            for t in times {
                pressure.advance(1000 + t, &mut out);
            }
            assert_eq!(pressure.current(), Some(([0.44, 0.38], 0.8)));
            pressure.input(Phase::Up, [0.44, 0.38], 0.8, 1350, &mut out);
            pressure.advance(1478, &mut out);
            assert!(pressure.current().is_none());
            assert!(matches!(out[0].1.phase, Phase::Down));
            assert!(matches!(out.last().unwrap().1.phase, Phase::Up));
            out.into_iter()
                .map(|(t, s)| (t, s.amount.unwrap()))
                .collect::<Vec<_>>()
        };
        let a = render(&[8, 24, 49, 123, 249, 349]);
        assert_eq!(a, render(&[349]));
        assert_eq!(
            (a[0], a[8], a[16], *a.last().unwrap()),
            ((1000, 0.), (1064, 0.4), (1128, 0.8), (1478, 0.))
        );
        assert_eq!(a.len(), 33);
        assert!(a.windows(2).all(|x| (x[0].1 - x[1].1).abs() < 0.076));
    }
}
