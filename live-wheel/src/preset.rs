use serde::{Deserialize, Serialize};

/// Which independently simulated pair of sealed films supplies the geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MotionPreset {
    #[default]
    WheelFlow,
    CoralRelease,
    WheelWithCoralBeads,
}
