use serde::{Deserialize, Serialize};

/// Viscosity-specific Basilisk trajectory for one sealed oil slide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum FilmOil {
    #[default]
    #[serde(rename = "wheel")]
    Wheel,
    #[serde(rename = "pdms-20")]
    Pdms20,
    #[serde(rename = "pdms-1000")]
    Pdms1000,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct MaterialMotion {
    pub rear: FilmOil,
    pub front: FilmOil,
}

/// Which independently simulated pair of sealed films supplies the geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MotionPreset {
    #[default]
    WheelFlow,
    CoralRelease,
    WheelWithCoralBeads,
}

#[cfg(test)]
mod material_tests {
    use super::{FilmOil, MaterialMotion};
    use serde_json::{from_value, json, to_value};

    #[test]
    fn omitted_side_uses_its_original_wheel_and_unknown_grades_are_rejected() {
        assert_eq!(
            from_value::<MaterialMotion>(json!({})).unwrap(),
            MaterialMotion::default()
        );
        let oils: MaterialMotion = from_value(json!({"rear":"pdms-1000"})).unwrap();
        assert_eq!(
            oils,
            MaterialMotion {
                rear: FilmOil::Pdms1000,
                front: FilmOil::Wheel
            }
        );
        assert_eq!(
            to_value(oils).unwrap(),
            json!({"rear":"pdms-1000","front":"wheel"})
        );
        assert!(from_value::<MaterialMotion>(json!({"front":"20"})).is_err());
    }
}
