use bytemuck::{Pod, Zeroable};
use serde::Deserialize;
use std::collections::HashMap;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Material {
    rear: [f32; 4],
    front: [f32; 4],
    joint: [f32; 4],
    rim: [f32; 4],
    carrier: [f32; 4],
    pub lens: [f32; 4],
}
#[cfg(test)]
impl Material {
    /// A unit-thickness film isolates geometric transmission from wheel depth variation.
    pub(crate) fn constant_film() -> Self {
        Self {
            rear: [0.24, 1.53, 0.49, 0.0],
            front: [5.02, 0.64, 0.165, 0.0],
            joint: [0.0; 4],
            rim: [0.0; 4],
            carrier: [1.0, 1.0, 1.0, 0.0],
            lens: [-1.0, 0.0, 0.0, 0.0],
        }
    }
}
#[derive(Deserialize)]
struct Registry {
    version: u32,
    wheels: Vec<Wheel>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wheel {
    id: String,
    swatches: Swatches,
    depth_variation: f32,
    rear_rim: Option<String>,
}
#[derive(Deserialize)]
struct Swatches {
    rear: String,
    front: String,
    carrier: String,
    overlap: String,
}

fn linear(s: &str) -> Result<[f32; 3], String> {
    if s.len() != 7 || !s.starts_with('#') || !s[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("Invalid wheel swatch {s}"));
    }
    let mut v = [0.0; 3];
    for (i, out) in v.iter_mut().enumerate() {
        let x = u8::from_str_radix(&s[1 + 2 * i..3 + 2 * i], 16).map_err(|e| e.to_string())? as f32
            / 255.0;
        *out = if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        };
    }
    Ok(v)
}
pub fn load() -> Result<HashMap<String, Material>, String> {
    let registry: Registry =
        serde_json::from_str(include_str!("../../WebAssets/assets/mathmos-wheels.json"))
            .map_err(|e| e.to_string())?;
    if registry.version != 1 {
        return Err("Unsupported Mathmos wheel registry".into());
    }
    let mut materials = HashMap::new();
    for w in registry.wheels {
        let c = linear(&w.swatches.carrier)?;
        let density = |s: &str| -> Result<[f32; 3], String> {
            let v = linear(s)?;
            Ok(std::array::from_fn(|i| {
                -(v[i] / c[i].max(0.00001)).clamp(0.00001, 1.0).ln()
            }))
        };
        let rear = density(&w.swatches.rear)?;
        let front = density(&w.swatches.front)?;
        let both = density(&w.swatches.overlap)?;
        let rim = w
            .rear_rim
            .as_deref()
            .map(density)
            .transpose()?
            .unwrap_or(rear);
        let mut m = Material::zeroed();
        m.lens[0] = -1.0;
        for i in 0..3 {
            m.rear[i] = rear[i];
            m.front[i] = front[i];
            m.carrier[i] = c[i];
            m.joint[i] = both[i].max(rear[i]).max(front[i]) - rear[i] - front[i];
            m.rim[i] = rim[i] - rear[i];
        }
        m.carrier[3] = w.depth_variation;
        if !(0.0..=1.0).contains(&w.depth_variation) || materials.insert(w.id, m).is_some() {
            return Err("Invalid or duplicate wheel".into());
        }
    }
    Ok(materials)
}
