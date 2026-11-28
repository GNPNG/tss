//! First-order projector conjugates. Distances and pupil radii are in mm.
//! The 1961 Kodak 550R specimen has a 5-inch f/3.5 Ektanar (Science Museum
//! Group 1990-5036/7646). Throw, gate size and layer spacing are scene inputs.
#[derive(Clone, Copy)]
pub(super) struct GateLens {
    pub rear: f32,
    pub front: f32,
    pub pupil_scale: f32,
}

impl GateLens {
    pub fn for_projector(carousel: bool) -> Self {
        if !carousel {
            return Self {
                rear: 53.4,
                front: 51.8,
                pupil_scale: 8. * 1600. * 1.8,
            };
        }
        let focal = 127.;
        let throw = 3000.;
        let object = focal * throw / (throw - focal);
        let pupil = focal / (2. * 3.5);
        let screen_height = 24. * throw / object;
        Self {
            rear: object + 0.8,
            front: object - 0.8,
            pupil_scale: pupil * throw * 720. / screen_height,
        }
    }
    pub fn distance(self, depth: f32) -> f32 {
        self.rear + (self.front - self.rear) * depth
    }
    pub fn radius(self, depth: f32, focus: f32) -> f32 {
        self.pupil_scale * (self.distance(depth).recip() - self.distance(focus).recip())
    }
    pub fn uniform(self) -> [f32; 4] {
        [self.rear, self.front, self.pupil_scale, 0.]
    }
}
