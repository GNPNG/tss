// Physical XY distances are expressed in visible gate heights (90 mm in the study).
const ASPECT: f64 = 16. / 9.;
const RADIUS: f64 = 0.32;
const MAX_COMPRESSION: f64 = 0.65;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(crate) struct Pressure {
    pub center: [f64; 2],
    radius: f64,
    amplitude: f64,
}

impl Pressure {
    pub fn new(center: [f64; 2], amount: f32, cache: [f32; 4]) -> Self {
        if amount == 0. {
            return Self::default();
        }
        let [sx, sy, ox, oy] = cache.map(f64::from);
        let [x, y] = center;
        // Clip the disk radius before it can touch a sealed edge. The material map
        // itself is never clipped; it is identity along the whole world boundary.
        let radius = RADIUS
            .min((x + ox / sx) * ASPECT)
            .min(((1. - ox) / sx - x) * ASPECT)
            .min(y + oy / sy)
            .min((1. - oy) / sy - y);
        if radius < 1e-6 {
            return Self::default();
        }
        Self {
            center,
            radius,
            amplitude: MAX_COMPRESSION * f64::from(amount),
        }
    }

    pub fn active(self) -> bool {
        self.amplitude > 0.
    }

    #[cfg(feature = "live-solver")]
    pub(super) fn squeeze(self, view: [f32; 4]) -> tss_basilisk_live::Squeeze {
        let [sx, sy, ox, oy] = view.map(f64::from);
        tss_basilisk_live::Squeeze {
            center: [self.center[0] * sx + ox, self.center[1] * sy + oy],
            radius: self.radius * sy,
            load: self.amplitude / MAX_COMPRESSION,
        }
    }

    #[cfg(feature = "live-solver")]
    pub(super) fn between(self, next: Self, fraction: f32) -> Self {
        let t = f64::from(fraction);
        let mut p = if self.active() { self } else { next };
        if self.active() && next.active() {
            for k in 0..2 { p.center[k] += (next.center[k] - p.center[k]) * t; }
            p.radius += (next.radius - p.radius) * t;
        }
        p.amplitude = self.amplitude + (next.amplitude - self.amplitude) * t;
        p
    }

    // Output-to-source coordinate and current gap / neutral physical gap.
    pub fn inverse(self, q: [f64; 2]) -> ([f64; 2], f64) {
        if !self.active() {
            return (q, 1.);
        }
        let d = [q[0] - self.center[0], q[1] - self.center[1]];
        let s = ((d[0] * ASPECT).powi(2) + d[1].powi(2)) / self.radius.powi(2);
        if s >= 1. {
            return (q, 1.);
        }
        let t = 1. - s;
        let k = self.amplitude * t * t;
        let scale = (1. - k * t).sqrt();
        (
            [self.center[0] + scale * d[0], self.center[1] + scale * d[1]],
            1. - k * (1. - 4. * s),
        )
    }

    pub fn shader(self, angle: f64) -> [f32; 4] {
        if !self.active() {
            return [0.; 4];
        }
        let (s, c) = angle.sin_cos();
        let x = (self.center[0] - 0.5) * ASPECT;
        let y = self.center[1] - 0.5;
        [
            (0.5 + (c * x - s * y) / ASPECT) as f32,
            (0.5 + s * x + c * y) as f32,
            (1. / self.radius.powi(2)) as f32,
            self.amplitude as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::{bundled_fields, Fields};

    #[test]
    fn inverse_area_matches_gap_at_every_radius_and_world_edges_stay_fixed() {
        let cache = [512. / 704., 288. / 704., 96. / 704., 208. / 704.];
        let mut max_error: f64 = 0.;
        for amount in [0.0, 0.17, 0.65, 1.] {
            let p = Pressure::new([0.46, 0.57], amount, cache);
            for i in 0..=200 {
                let x = p.center[0] + RADIUS / ASPECT * i as f64 / 170.;
                let y = p.center[1];
                let e = 1e-6;
                let a = p.inverse([x - e, y]).0;
                let b = p.inverse([x + e, y]).0;
                let c = p.inverse([x, y - e]).0;
                let d = p.inverse([x, y + e]).0;
                let jac =
                    ((b[0] - a[0]) * (d[1] - c[1]) - (b[1] - a[1]) * (d[0] - c[0])) / (4. * e * e);
                let h = p.inverse([x, y]).1;
                max_error = max_error.max((jac - h).abs());
                assert!((0.35 - 1e-10..=1.1625 + 1e-10).contains(&h));
            }
        }
        println!("max gap/det derivative difference {max_error:.9}");
        assert!(max_error < 1e-8);
        let near = Pressure::new([0.01, 0.12], 1., [1., 1., 0., 0.]);
        for y in (0..=100).map(|n| n as f64 / 100.) {
            assert_eq!(near.inverse([0., y]), ([0., y], 1.));
        }
        assert_eq!(
            Pressure::new([0., 0.], 1., [1., 1., 0., 0.]).active(),
            false
        );
    }

    fn coverage(f: &Fields, data: &[u8], uv: [f64; 2], slide: usize) -> f64 {
        let [w, h] = f.size;
        let [x, y] = [uv[0] * f64::from(w) - 0.5, uv[1] * f64::from(h) - 0.5];
        let load = |i: i32, j: i32| {
            f64::from(
                data[2
                    * (j.clamp(0, h as i32 - 1) as usize * w as usize
                        + i.clamp(0, w as i32 - 1) as usize)
                    + slide],
            )
        };
        let (i, j) = (x.floor() as i32, y.floor() as i32);
        let (a, b) = (x - x.floor(), y - y.floor());
        let sample = (1. - b) * ((1. - a) * load(i, j) + a * load(i + 1, j))
            + b * ((1. - a) * load(i, j + 1) + a * load(i + 1, j + 1));
        // Fractional interface of width ~1.1 design pixels, shared by both integrals.
        ((sample - 127.5) / 3. + 0.5).clamp(0., 1.)
    }

    #[test]
    fn sealed_world_dye_and_carrier_volumes_survive_a_press() {
        let f = Fields::from_slice(&bundled_fields().unwrap()).unwrap();
        let data = f.frame(30);
        let cache = f.view_to_cache();
        let [sx, sy, ox, oy] = cache.map(f64::from);
        let n = f.size[0] * 2;
        let samples = f64::from(n).powi(2);
        for (slide, center) in [[0.365, 0.48], [0.58, 0.41]].into_iter().enumerate() {
            let p = Pressure::new(center, 1., cache);
            let mut base = [0., 0.];
            let mut after = [0., 0.];
            let mut dyed_area = 0.;
            for j in 0..n {
                for i in 0..n {
                    let atlas = [
                        (f64::from(i) + 0.5) / f64::from(n),
                        (f64::from(j) + 0.5) / f64::from(n),
                    ];
                    let gate = [(atlas[0] - ox) / sx, (atlas[1] - oy) / sy];
                    let (q, gap) = p.inverse(gate);
                    let x = coverage(&f, data, atlas, slide);
                    let y = coverage(&f, data, [q[0] * sx + ox, q[1] * sy + oy], slide);
                    base[0] += x;
                    base[1] += 1. - x;
                    after[0] += gap * y;
                    after[1] += gap * (1. - y);
                    dyed_area += y;
                }
            }
            let error = [
                (after[0] - base[0]) / base[0],
                (after[1] - base[1]) / base[1],
            ];
            println!("slide {slide}: normalized neutral {:.9}/{:.9}, compressed {:.9}/{:.9}; relative dye/carrier errors {:.9}/{:.9}; dyed area changed {:.4}% relative",
                     base[0]/samples,base[1]/samples,after[0]/samples,after[1]/samples,error[0],error[1],100.*(dyed_area/base[0]-1.));
            assert!((dyed_area / base[0] - 1.).abs() > 0.001);
            assert!(error.iter().all(|v| v.abs() < 0.0001));
        }
    }
}
