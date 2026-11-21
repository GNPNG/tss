use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    #[default]
    Visualizer,
    DspCover,
    Vertical,
    Panorama,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Encoding {
    #[default]
    Rec709,
    Srgb,
    DisplayP3,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct DigitalFormat {
    pub layout: Layout,
    pub encoding: Encoding,
    pub horizon_y: Option<u32>,
}

impl DigitalFormat {
    pub fn validate(self) -> Result<(), String> {
        use Encoding::*;
        use Layout::*;
        if !matches!(
            (self.layout, self.encoding),
            (Visualizer, Rec709 | DisplayP3)
                | (DspCover, Srgb)
                | (Vertical, Srgb | Rec709)
                | (Panorama, Srgb | DisplayP3)
        ) {
            return Err("Color encoding is incompatible with this DIGITAL_4K format".into());
        }
        if self
            .horizon_y
            .is_some_and(|y| self.layout != Visualizer || y >= 2160)
        {
            return Err("The optional horizon must lie inside the 2160-pixel visualizer".into());
        }
        Ok(())
    }

    pub fn size(self) -> [u32; 2] {
        match self.layout {
            Layout::Visualizer => [3840, 2160],
            Layout::DspCover => [4000, 4000],
            Layout::Vertical => [2160, 3840],
            Layout::Panorama => [10800, 2160],
        }
    }
}
