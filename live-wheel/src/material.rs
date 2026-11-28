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
    pub lamp: [f32; 4],
    pub geometry: [f32; 4],
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
            lamp: [1.; 4],
            geometry: crate::lens::GateLens::for_projector(false).uniform(),
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
        return Err(format!("Invalid slide swatch {s}"));
    }
    srgb(u32::from_str_radix(&s[1..], 16).map_err(|e| e.to_string())?)
}

fn srgb(rgb: u32) -> Result<[f32; 3], String> {
    if rgb > 0xffffff {
        return Err("Film swatches must be 24-bit sRGB".into());
    }
    Ok([16, 8, 0].map(|shift| {
        let v = ((rgb >> shift) & 255) as f32 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }))
}

fn optical_density(light: f32, carrier: f32) -> f32 {
    -(light / carrier.max(0.00001)).clamp(0.00001, 1.0).ln()
}

/// An independently exposed cell filters a neutral lamp in its own optical pass.
/// All palette families use the rear slot for the cell's dye.
pub(super) fn single(
    mut profile: Material,
    swatches: Option<[u32; 4]>,
) -> Result<Material, String> {
    if let Some([_, dye, _, _]) = swatches {
        for (i, value) in srgb(dye)?.into_iter().enumerate() {
            profile.rear[i] = optical_density(value, 1.);
        }
    }
    profile.carrier[..3].fill(1.);
    profile.joint = [0.; 4];
    profile.front = [0.; 4];
    profile.rim = [0.; 4];
    profile.rear[3] = 0.;
    Ok(profile)
}

/// Resolve dye densities once when a track is selected or edited.
pub fn resolve(
    presets: &HashMap<String, Material>,
    id: &str,
    swatches: Option<[u32; 4]>,
) -> Result<Material, String> {
    let base = presets.get(id);
    let Some(colors) = swatches else {
        return base
            .copied()
            .ok_or_else(|| format!("Missing optical profile or swatches for {id}"));
    };
    let [a, b, c, d] = colors.map(srgb);
    let [a, b, c, d] = [a?, b?, c?, d?];
    if let Some(base) = base {
        // Wheel editor order: clear carrier, rear, photographed overlap, front.
        // Keep the calibrated meniscus shift and thickness variation of this wheel.
        let mut m = *base;
        for i in 0..3 {
            m.carrier[i] = a[i];
            let rear = optical_density(b[i], a[i]);
            let front = optical_density(d[i], a[i]);
            let both = optical_density(c[i], a[i]);
            m.rear[i] = rear;
            m.front[i] = front;
            m.rim[i] = base.rim[i].max(-rear);
            m.joint[i] = both.max(rear).max(front) - rear - front;
        }
        return Ok(m);
    }
    if !matches!(
        id,
        "orchid" | "lagoon" | "ember" | "twilight" | "sunrise" | "meadow"
    ) {
        return Err(format!("Unknown optical palette {id}"));
    }
    // Authored theme order: rear oil, front oil, rear meniscus, front meniscus.
    // A clear carrier transmits the projector lamp and overlapping films multiply.
    let mut m = Material::zeroed();
    m.lamp = [1.; 4];
    m.geometry = crate::lens::GateLens::for_projector(false).uniform();
    m.carrier = [1.; 4];
    m.lens[0] = -1.;
    for i in 0..3 {
        m.rear[i] = optical_density(a[i], 1.);
        m.front[i] = optical_density(b[i], 1.);
        m.rim[i] = optical_density(c[i], 1.) - m.rear[i];
    }
    let front_rim: [f32; 3] = std::array::from_fn(|i| optical_density(d[i], 1.) - m.front[i]);
    // The three unused std140 padding components keep existing material buffers compatible.
    (m.rear[3], m.front[3], m.joint[3]) = (front_rim[0], front_rim[1], front_rim[2]);
    Ok(m)
}

pub fn load() -> Result<HashMap<String, Material>, String> {
    let registry: Registry =
        serde_json::from_str(include_str!("../../WebAssets/assets/slide-presets.json"))
            .map_err(|e| e.to_string())?;
    if registry.version != 1 {
        return Err("Unsupported slide preset registry".into());
    }
    let mut materials = HashMap::new();
    for w in registry.wheels {
        let c = linear(&w.swatches.carrier)?;
        let density = |s: &str| -> Result<[f32; 3], String> {
            let v = linear(s)?;
            Ok(std::array::from_fn(|i| optical_density(v[i], c[i])))
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
        m.lamp = [1.; 4];
        m.geometry = crate::lens::GateLens::for_projector(false).uniform();
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
            return Err("Invalid or duplicate slide".into());
        }
    }
    Ok(materials)
}

#[cfg(test)]
mod authored_palette_tests {
    use super::*;

    #[test]
    fn all_themes_have_independent_body_and_meniscus_density_per_film() {
        let presets = load().unwrap();
        let colors = [0xd72c95, 0x9cdb27, 0x048f95, 0x6931ac];
        for id in ["orchid", "lagoon", "ember", "twilight", "sunrise", "meadow"] {
            let m = resolve(&presets, id, Some(colors)).unwrap();
            let front_rim = [m.rear[3], m.front[3], m.joint[3]];
            for i in 0..3 {
                for (density, color) in [
                    (m.rear[i], colors[0]),
                    (m.front[i], colors[1]),
                    (m.rear[i] + m.rim[i], colors[2]),
                    (m.front[i] + front_rim[i], colors[3]),
                ] {
                    assert!(((-density).exp() - srgb(color).unwrap()[i]).abs() < 1e-5);
                }
                assert_eq!(m.carrier[i], 1.);
                assert_eq!(m.joint[i], 0.);
            }
        }
        assert!(resolve(&presets, "unknown", Some(colors)).is_err());
        assert!(resolve(&presets, "orchid", Some([0x1000000; 4])).is_err());
    }

    #[test]
    fn photographed_slide_preserves_calibration_and_custom_clear_rear_overlap_front_order() {
        let presets = load().unwrap();
        for (id, expected) in &presets {
            let actual = resolve(&presets, id, None).unwrap();
            assert_eq!(bytemuck::bytes_of(&actual), bytemuck::bytes_of(expected));
        }
        let m = resolve(
            &presets,
            "blue-red",
            Some([0xffffff, 0xdd8866, 0x100d10, 0x99ccee]),
        )
        .unwrap();
        for i in 0..3 {
            let expected = srgb(0x100d10).unwrap();
            assert!(((-(m.rear[i] + m.front[i] + m.joint[i])).exp() - expected[i]).abs() < 1e-5);
        }
        assert_eq!(m.carrier[3], presets["blue-red"].carrier[3]);
    }
}
