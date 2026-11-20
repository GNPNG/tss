// Independent kinematic state for each sealed slide, in film coordinates.
// Distances use the visible film height as one physical unit.
const ASPECT: f64 = 16. / 9.;
pub(crate) const TUG_SIDE: f64 = 0.64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideGesture {
    Tug,
    Rotate,
}
#[derive(Clone, Copy)]
pub(crate) struct SlidePose {
    pub angle: f64,
    pub center: [f64; 2],
    pub strength: [f64; 2],
    pub response: f32,
    start: Option<[f64; 2]>,
}
impl Default for SlidePose {
    fn default() -> Self {
        Self {
            angle: 0.,
            center: [0.5; 2],
            strength: [0.; 2],
            response: 0.40,
            start: None,
        }
    }
}
impl SlidePose {
    pub fn inverse_rotation(&self) -> [f32; 2] {
        let (s, c) = (-self.angle).sin_cos();
        [c as f32, s as f32]
    }
    pub fn material_point(&self, p: [f64; 2]) -> [f64; 2] {
        let (s, c) = (-self.angle).sin_cos();
        let x = (p[0] - 0.5) * ASPECT;
        let y = p[1] - 0.5;
        [0.5 + (c * x - s * y) / ASPECT, 0.5 + s * x + c * y]
    }
    pub fn end(&mut self) {
        self.start = None;
    }
    pub fn gesture(&mut self, mode: SlideGesture, from: [f32; 2], to: [f32; 2]) -> bool {
        let from = from.map(f64::from);
        let to = to.map(f64::from);
        if mode == SlideGesture::Rotate {
            self.end();
            let a = [(from[0] - 0.5) * ASPECT, from[1] - 0.5];
            let b = [(to[0] - 0.5) * ASPECT, to[1] - 0.5];
            if a[0].hypot(a[1]) < 0.015 || b[0].hypot(b[1]) < 0.015 {
                return false;
            }
            let delta = (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]);
            self.angle = (self.angle + delta).rem_euclid(std::f64::consts::TAU);
            return false; // rigid pose is sent separately in the per-frame uniform
        }
        let a = self.material_point(from);
        let b = self.material_point(to);
        let anchor = *self.start.get_or_insert(a);
        let dx = (b[0] - anchor[0]) * ASPECT;
        let dy = b[1] - anchor[1];
        let distance = dx.hypot(dy);
        let step = ((b[0] - a[0]) * ASPECT).hypot(b[1] - a[1]);
        let response = f64::from(self.response);
        let follow = 1. - (-step / (0.120 - 0.085 * response)).exp();
        let scale = if distance > 0. {
            (0.13 + 0.29 * response) * (distance / 0.115).tanh() / distance
        } else {
            0.
        };
        let target = [dx * scale, dy * scale];
        let pristine = self.strength == [0., 0.];
        for i in 0..2 {
            self.center[i] = if pristine {
                anchor[i]
            } else {
                self.center[i] + follow * (anchor[i] - self.center[i])
            };
            self.strength[i] += follow * (target[i] - self.strength[i]);
        }
        true
    }
}
