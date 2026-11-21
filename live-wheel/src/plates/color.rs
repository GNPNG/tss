use super::{rec709, Encoding};

impl Encoding {
    pub(super) fn working_rgb(self, p: [f32; 4]) -> [f32; 4] {
        if self != Self::DisplayP3 {
            return p;
        }
        // W3C CSS Color 4 D65 linear-sRGB -> XYZ -> linear Display P3.
        let [r, g, b, a] = p;
        [
            0.82246197 * r + 0.17753803 * g,
            0.03319420 * r + 0.96680580 * g,
            0.01708263 * r + 0.07239744 * g + 0.91051993 * b,
            a,
        ]
    }

    pub(super) fn quantize(self, value: f32, alpha: bool) -> u16 {
        if alpha || self == Self::Rec709 {
            return rec709::quantize(value, alpha);
        }
        let v = f64::from(value.clamp(0., 1.));
        let encoded = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1. / 2.4) - 0.055
        };
        (encoded * 65535.).round() as u16
    }

    pub(super) fn cicp(self) -> [u8; 4] {
        match self {
            Self::Rec709 => [1, 1, 0, 1],
            Self::Srgb => [1, 13, 0, 1],
            Self::DisplayP3 => [12, 13, 0, 1],
        }
    }

    pub(super) fn icc(self) -> &'static [u8] {
        match self {
            Self::Rec709 => include_bytes!("../../assets/color/Rec709-v4.icc"),
            Self::Srgb => include_bytes!("../../assets/color/sRGB-v4.icc"),
            Self::DisplayP3 => include_bytes!("../../assets/color/DisplayP3-v4.icc"),
        }
    }
}
