/// Quantize a finite linear sample with the same transfer used by the editor.
pub fn quantize(sample: f32, alpha: bool) -> u16 {
    let value = sample.clamp(0.0, 1.0) as f64;
    let encoded = if alpha {
        value
    } else if value < 0.018 {
        4.5 * value
    } else {
        1.099 * value.powf(0.45) - 0.099
    };
    (encoded.clamp(0.0, 1.0) * 65535.0).round() as u16
}
