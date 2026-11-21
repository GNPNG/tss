use super::*;
use crate::production::Mechanical;
use flate2::{write::ZlibEncoder, Compression};
use pdf_writer::{
    types::{OutputIntentSubtype, OverprintMode},
    writers::ColorSpace,
    Content, Filter, Finish, Name, Pdf, Rect, Ref, TextStr,
};
use ttf_parser::{Face, OutlineBuilder};

pub(super) struct Stencil {
    width: usize,
    bits: Vec<u8>,
}
impl Stencil {
    pub(super) fn read(data: Vec<u8>, size: [u32; 2]) -> Result<Self, String> {
        let mut reader = png::Decoder::new(std::io::Cursor::new(data))
            .read_info()
            .map_err(|e| e.to_string())?;
        let info = reader.info();
        if [info.width, info.height] != size
            || info.color_type != png::ColorType::Grayscale
            || info.bit_depth != png::BitDepth::One
            || info.trns.is_some()
            || info.animation_control.is_some()
        {
            return Err(format!("Choose an opaque, single-frame, 1-bit grayscale PNG at {} × {}. Black pixels print; white pixels stay clear.",size[0],size[1]));
        }
        let width = size[0] as usize;
        let mut bits = vec![0; width.div_ceil(8) * size[1] as usize];
        reader.next_frame(&mut bits).map_err(|e| e.to_string())?;
        Ok(Self { width, bits })
    }
    pub(super) fn marks(&self, index: usize) -> bool {
        let x = index % self.width;
        let y = index / self.width;
        self.bits[y * self.width.div_ceil(8) + x / 8] & (0x80 >> (x % 8)) == 0
    }
}

fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut stream = ZlibEncoder::new(Vec::new(), Compression::fast());
    stream.write_all(data).map_err(|e| e.to_string())?;
    stream.finish().map_err(|e| e.to_string())
}

const POINTS_PER_MM: f32 = 72. / 25.4;
const TYPE_SIZE_PT: f32 = 6.5;
const FACE: &[u8] = include_bytes!("../../../assets/type/SourceSans3-Regular.otf");

struct Glyph<'a> {
    content: &'a mut Content,
    pen: [f32; 2],
}
impl OutlineBuilder for Glyph<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.content.move_to(x, y);
        self.pen = [x, y];
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.content.line_to(x, y);
        self.pen = [x, y];
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let [px, py] = self.pen;
        self.curve_to(
            px + 2. * (cx - px) / 3.,
            py + 2. * (cy - py) / 3.,
            x + 2. * (cx - x) / 3.,
            y + 2. * (cy - y) / 3.,
            x,
            y,
        );
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.content.cubic_to(a, b, c, d, x, y);
        self.pen = [x, y];
    }
    fn close(&mut self) {
        self.content.close_path();
    }
}

// Place outlined, positive-reading plate type on an explicit uninked panel.
// This keeps K-only overprint legible and its physical TAC at exactly 100%.
fn colophon(content: &mut Content, spec: &Mechanical, catalog: u16) -> Result<(), String> {
    if catalog > 999 {
        return Err("EPNI catalog must be 000–999".into());
    }
    let font = Face::parse(FACE, 0).map_err(|e| e.to_string())?;
    let scale = TYPE_SIZE_PT / f32::from(font.units_per_em());
    let catalog = format!("EPNI-{catalog:03}");
    let lines = [
        "Bijon Roy · Brett Whitty · Jamie Mellway",
        "The Whitty Compound",
        &catalog,
    ];
    let width = |text: &str| {
        text.chars()
            .filter_map(|c| font.glyph_index(c))
            .filter_map(|g| font.glyph_hor_advance(g))
            .map(f32::from)
            .sum::<f32>()
            * scale
    };
    let widths = lines.map(width);
    let panel_w = widths.iter().copied().fold(0_f32, f32::max) + 4.;
    let panel_h = 27.;
    let [w, h] = spec.canvas_mm();
    let (cx, y) = if let Some(hole) = spec.hole_diameter_mm {
        let outer = spec.trim_mm[0] / 2. - spec.safe_mm;
        let inner = hole / 2. + spec.safe_mm;
        // Centre the panel in the lower portion of the safe annulus, checking
        // both the outer corners and the nearest edge to the spindle.
        let bottom = (outer.powi(2) - (panel_w / 2. / POINTS_PER_MM).powi(2)).sqrt();
        let available = (bottom - inner) * POINTS_PER_MM;
        if available < panel_h {
            return Err("Colophon exceeds the label's safe annulus".into());
        }
        (
            w * POINTS_PER_MM / 2.,
            (h / 2. - bottom) * POINTS_PER_MM + (available - panel_h) / 2.,
        )
    } else {
        let panel = spec
            .spine_mm
            .map_or(spec.trim_mm[0], |s| (spec.trim_mm[0] - s) / 2.);
        if panel_w > (panel - 2. * spec.safe_mm) * POINTS_PER_MM {
            return Err("Colophon exceeds safe type width".into());
        }
        (
            (spec.bleed_mm + spec.safe_mm) * POINTS_PER_MM + panel_w / 2.,
            (spec.bleed_mm + spec.safe_mm) * POINTS_PER_MM,
        )
    };
    content
        .save_state()
        .set_fill_cmyk(0., 0., 0., 0.)
        .rect(cx - panel_w / 2., y, panel_w, panel_h)
        .fill_nonzero();
    content
        .set_parameters(Name(b"Overprint"))
        .set_fill_cmyk(0., 0., 0., 1.);
    for (n, text) in lines.iter().enumerate() {
        let mut x = cx - widths[n] / 2.;
        let baseline = y + 19. - n as f32 * 7.7;
        for c in text.chars() {
            let id = font.glyph_index(c).ok_or("Missing colophon glyph")?;
            content
                .save_state()
                .transform([scale, 0., 0., scale, x, baseline]);
            font.outline_glyph(
                id,
                &mut Glyph {
                    content,
                    pen: [0.; 2],
                },
            );
            content.fill_nonzero().restore_state();
            x += f32::from(font.glyph_hor_advance(id).ok_or("Missing glyph advance")?) * scale;
        }
    }
    content.restore_state();
    Ok(())
}

pub(super) fn write_pdf(
    path: &Path,
    format: PrintFormat,
    profile: &PrintProfile,
    process_zlib: &[u8],
) -> Result<(), String> {
    let spec = format.layout.mechanical();
    let [w, h] = spec.canvas_mm().map(|v| v * POINTS_PER_MM);
    let bleed = spec.bleed_mm * POINTS_PER_MM;
    let [pw, ph] = spec.pixels.map(|v| v as i32);
    let [catalog, tree, page_id, contents, image, icc, mask, op] =
        [1, 2, 3, 4, 5, 6, 7, 8].map(Ref::new);
    let mut pdf = Pdf::new();
    pdf.set_version(1, 7);
    let mut root = pdf.catalog(catalog);
    root.pages(tree)
        .output_intents()
        .push()
        .subtype(OutputIntentSubtype::PDFX)
        .output_condition_identifier(TextStr(format.stock.profile_name()))
        .dest_output_profile(icc);
    root.finish();
    pdf.pages(tree).kids([page_id]).count(1);
    let mut page = pdf.page(page_id);
    let canvas = Rect::new(0., 0., w, h);
    page.parent(tree)
        .media_box(canvas)
        .bleed_box(canvas)
        .trim_box(Rect::new(bleed, bleed, w - bleed, h - bleed))
        .contents(contents);
    let mut resources = page.resources();
    let mut images = resources.x_objects();
    images.pair(Name(b"Process"), image);
    if format.gilding.is_some() {
        images.pair(Name(b"Stencil"), mask);
    }
    images.finish();
    resources.ext_g_states().pair(Name(b"Overprint"), op);
    if let Some(gilding) = format.gilding {
        let mut colors = resources.color_spaces();
        let mut spot = colors
            .insert(Name(b"Gold"))
            .start::<ColorSpace>()
            .separation(Name(gilding.plate().as_bytes()));
        // Alternate preview only. The plate is identified by the actual named colorant.
        spot.alternate_color_space().device_cmyk();
        spot.tint_exponential()
            .domain([0., 1.])
            .n(1.)
            .c0([0., 0., 0., 0.])
            .c1([0.2, 0.3, 0.6, 0.2]);
    }
    resources.finish();
    page.finish();
    pdf.icc_profile(icc, &profile.bytes)
        .n(4)
        .alternate()
        .device_cmyk();
    let mut picture = pdf.image_xobject(image, process_zlib);
    picture.filter(Filter::FlateDecode);
    picture.width(pw).height(ph).bits_per_component(16);
    picture.color_space().device_cmyk();
    picture.finish();
    pdf.ext_graphics(op)
        .overprint(true)
        .overprint_fill(true)
        .overprint_mode(OverprintMode::IgnoreZeroChannel);
    let mut content = Content::new();
    content
        .save_state()
        .transform([w, 0., 0., h, 0., 0.])
        .x_object(Name(b"Process"))
        .restore_state();
    if let Some(catalog) = format.catalog {
        colophon(&mut content, &spec, catalog)?;
    }
    if let Some(stencil) = &profile.spot {
        let bytes = compress(&stencil.bits)?;
        let mut image = pdf.image_xobject(mask, &bytes);
        image.filter(Filter::FlateDecode);
        image
            .width(pw)
            .height(ph)
            .image_mask(true)
            .bits_per_component(1);
        image.decode([0., 1.]);
        image.finish();
        content
            .save_state()
            .set_parameters(Name(b"Overprint"))
            .set_fill_color_space(Name(b"Gold"))
            .set_fill_color([1.])
            .transform([w, 0., 0., h, 0., 0.])
            .x_object(Name(b"Stencil"))
            .restore_state();
    }
    pdf.stream(contents, &content.finish());
    fs::write(path, pdf.finish()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::PrintLayout::*;
    #[test]
    fn outlined_colophon_fits_each_mechanical_including_the_annular_labels() {
        for layout in [
            LpJacket,
            LpGatefold,
            PictureSleeve,
            Label7,
            Label12,
            CdGatefold,
            Obi,
        ] {
            let mut c = Content::new();
            colophon(&mut c, &layout.mechanical(), 10).unwrap();
            assert!(c.finish().windows(9).any(|v| v == b"0 0 0 1 k"));
        }
    }
}
