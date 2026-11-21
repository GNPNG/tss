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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PrintLayout {
    LpJacket,
    LpGatefold,
    PictureSleeve,
    Label7,
    Label12,
    CdGatefold,
    Obi,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Stock {
    Coated,
    Uncoated,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PrintFormat {
    pub layout: PrintLayout,
    pub stock: Stock,
    pub catalog: Option<u16>,
    pub gilding: Option<Gilding>,
    #[serde(default)]
    pub rich_black: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mechanical {
    pub pixels: [u32; 2],
    pub trim_mm: [f32; 2],
    pub bleed_mm: f32,
    pub safe_mm: f32,
    pub spine_mm: Option<f32>,
    pub hole_diameter_mm: Option<f32>,
}

impl PrintLayout {
    pub fn mechanical(self) -> Mechanical {
        use PrintLayout::*;
        let (pixels, trim_mm, bleed_mm, safe_mm, spine_mm, hole_diameter_mm) = match self {
            LpJacket => ([3780, 3780], [314., 314.], 3., 6., None, None),
            LpGatefold => ([7559, 3780], [634., 314.], 3., 6., Some(6.), None),
            PictureSleeve => ([2244, 2244], [184., 184.], 3., 5., None, None),
            Label7 => ([1134, 1134], [92., 92.], 2., 4., None, Some(38.1)),
            Label12 => ([1228, 1228], [100., 100.], 2., 4., None, Some(7.24)),
            CdGatefold => ([3319, 1665], [275., 135.], 3., 4.5, Some(5.), None),
            Obi => ([839, 3780], [65., 314.], 3., 4., None, None),
        };
        Mechanical {
            pixels,
            trim_mm,
            bleed_mm,
            safe_mm,
            spine_mm,
            hole_diameter_mm,
        }
    }
}

impl Mechanical {
    pub fn canvas_mm(&self) -> [f32; 2] {
        self.trim_mm.map(|v| v + 2. * self.bleed_mm)
    }
}

impl Stock {
    pub fn profile_name(self) -> &'static str {
        match self {
            Self::Coated => "ISOcoated_v2_300_eci.icc",
            Self::Uncoated => "PSO_Uncoated_ISO12647_eci.icc",
        }
    }

    pub fn tac(self) -> u32 {
        match self {
            Self::Coated => 300,
            Self::Uncoated => 280,
        }
    }
}

impl Mechanical {
    // Rectangles, die circles and safe boundaries are in physical units. The
    // finishing layer is separate from the full-bleed process-color TIFF.
    pub fn guide_svg(&self) -> String {
        use std::fmt::Write;
        let [w, h] = self.canvas_mm();
        let [tw, th] = self.trim_mm;
        let b = self.bleed_mm;
        let s = self.safe_mm;
        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}"><title>Non-printing finishing guides in millimetres</title><g fill="none" stroke-width="0.25"><rect id="bleed" width="{w}" height="{h}" stroke="#2082c4"/>"##
        );
        if let Some(hole) = self.hole_diameter_mm {
            let (cx, cy) = (w / 2., h / 2.);
            for (name, r, color) in [
                ("outer-die", tw / 2., "#d03754"),
                ("spindle-die", hole / 2., "#d03754"),
                ("outer-safe", tw / 2. - s, "#439667"),
                ("inner-safe", hole / 2. + s, "#439667"),
            ] {
                write!(
                    svg,
                    r#"<circle id="{name}" cx="{cx}" cy="{cy}" r="{r}" stroke="{color}"/>"#
                )
                .unwrap();
            }
        } else {
            write!(
                svg,
                r##"<rect id="trim" x="{b}" y="{b}" width="{tw}" height="{th}" stroke="#d03754"/>"##
            )
            .unwrap();
            let (x, y, height) = (b + s, b + s, th - 2. * s);
            let panel_width = self.spine_mm.map_or(tw, |spine| (tw - spine) / 2.);
            let width = panel_width - 2. * s;
            write!(svg,r##"<rect id="safe" x="{x}" y="{y}" width="{width}" height="{height}" stroke="#439667"/>"##).unwrap();
            if let Some(spine) = self.spine_mm {
                let left = b + panel_width;
                let right = left + spine;
                let safe_right = right + s;
                let bottom = h - b;
                write!(svg,r##"<path id="folds" d="M {left} {b} V {bottom} M {right} {b} V {bottom}" stroke="#9372c4"/><rect id="safe-right" x="{safe_right}" y="{y}" width="{width}" height="{height}" stroke="#439667"/>"##).unwrap();
            }
        }
        svg.push_str("</g></svg>");
        svg
    }
}

#[cfg(test)]
mod print_tests {
    use super::*;
    #[test]
    fn physical_dimensions_agree_with_the_300_ppi_raster_and_finishing_guides() {
        use PrintLayout::*;
        for layout in [
            LpJacket,
            LpGatefold,
            PictureSleeve,
            Label7,
            Label12,
            CdGatefold,
            Obi,
        ] {
            let spec = layout.mechanical();
            for (px, mm) in spec.pixels.into_iter().zip(spec.canvas_mm()) {
                assert!((px as f32 - mm / 25.4 * 300.).abs() <= 0.51);
            }
            let svg = spec.guide_svg();
            assert_eq!(svg.contains("spindle-die"), spec.hole_diameter_mm.is_some());
            assert_eq!(svg.contains("safe-right"), spec.spine_mm.is_some());
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Gilding {
    Metallic,
    Foil,
}

impl Gilding {
    pub fn plate(self) -> &'static str {
        match self {
            Self::Metallic => "PANTONE 871 C",
            Self::Foil => "FOIL_GOLD",
        }
    }
}
