use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaMode {
    Color,
    Key,
    Silhouette,
    Gradient,
    Rainbow,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cutout {
    None,
    Circle,
    Diamond,
    Star,
    #[serde(rename = "star-tunnel")]
    StarTunnel,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaskOp {
    Intersect,
    Union,
    Subtract,
    Xor,
}

/// Media source timing and artwork settings at the native scene boundary.
/// Colors are normalized sRGB triples, as in the editor's color picker.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSettings {
    pub asset_id: u64,
    pub duration_us: i64,
    pub still: bool,
    pub offset_us: i64,
    pub rate: f64,
    #[serde(rename = "loop")]
    pub looping: bool,
    pub mode: MediaMode,
    #[serde(default)]
    pub key_enabled: bool,
    pub key_color: [f32; 3],
    pub key_threshold: f32,
    pub key_softness: f32,
    pub gradient_a: [f32; 3],
    pub gradient_b: [f32; 3],
    pub gradient_rate: f32,
    pub bounce: f32,
    pub blur: f32,
    pub distortion: f32,
    pub mask_a: Option<u64>,
    pub mask_b: Option<u64>,
    pub mask_op: MaskOp,
    pub shape: Cutout,
    #[serde(default = "unit_speed")]
    pub tunnel_speed: f32,
    #[serde(default)]
    pub tunnel_bass: f32,
}
fn unit_speed() -> f32 {
    1.
}
impl MediaSettings {
    pub fn valid(&self) -> bool {
        self.asset_id > 0
            && self.duration_us >= 0
            && (self.still || self.duration_us > 0)
            && self.offset_us >= 0
            && (0.0..=8.).contains(&self.rate)
            && (0.0..=12.).contains(&self.gradient_rate)
            && (0.0..=8.).contains(&self.tunnel_speed)
            && (0.0..=1.).contains(&self.tunnel_bass)
            && self
                .key_color
                .iter()
                .chain(&self.gradient_a)
                .chain(&self.gradient_b)
                .chain([
                    &self.key_threshold,
                    &self.key_softness,
                    &self.bounce,
                    &self.blur,
                    &self.distortion,
                ])
                .all(|x| (0.0..=1.).contains(x))
    }
    pub fn source_time(&self, program: f64, envelope: Option<&super::OpacityEnvelope>) -> i64 {
        if self.still {
            return 0;
        }
        let global = |time: f64| {
            self.offset_us
                .saturating_add((time.max(0.) * self.rate * 1e6).round() as i64)
        };
        let t = match envelope.and_then(|e| e.region_at(program)) {
            Some(r) if r.media_start_us.is_some() || r.media_rate.is_some() => {
                let origin = r.media_start_us.unwrap_or_else(|| global(r.start));
                origin.saturating_add(
                    ((program - r.start) * r.media_rate.unwrap_or(self.rate) * 1e6).round() as i64,
                )
            }
            _ => global(program),
        };
        if self.looping {
            t.rem_euclid(self.duration_us)
        } else {
            t.min(self.duration_us - 1)
        }
    }
}
