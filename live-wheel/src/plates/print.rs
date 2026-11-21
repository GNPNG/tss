//! Native selected-wheel PRINT_CMYK separation. The original destination ICC
//! accompanies every TIFF. The stock-specific device link sets GCR and TAC.
use super::*;
mod finishing;
use crate::production::{PrintFormat, Stock};
use finishing::Stencil;
use lcms2::{Flags, Intent, PixelFormat, Profile, Transform};
use sha2::{Digest, Sha256};
use tiff::{
    encoder::{colortype::CMYK16, Compression, Rational, TiffEncoder, TiffValue},
    tags::{Predictor, ResolutionUnit, Tag, Type},
};

pub struct PrintProfile {
    bytes: Vec<u8>,
    stock: Stock,
    spot: Option<Stencil>,
    rich: Option<Stencil>,
}

impl PrintProfile {
    pub fn new(stock: Stock, bytes: Vec<u8>) -> Result<Self, String> {
        let expected = match stock {
            Stock::Coated => "c6b4b62f0726243742eced8b9669476a6be89e581f50a7600ed8b6fcbb9cdab8",
            Stock::Uncoated => "e170a59fbf26e7845c62b07e95901b1ef3335a664c03e7ef48ddb84fa08337be",
        };
        if format!("{:x}", Sha256::digest(&bytes)) != expected {
            return Err(format!(
                "Choose the original ECI Offset 2009 {} profile for this stock",
                stock.profile_name()
            ));
        }
        Ok(Self {
            bytes,
            stock,
            spot: None,
            rich: None,
        })
    }

    pub fn set_masks(
        &mut self,
        format: PrintFormat,
        spot: Option<Vec<u8>>,
        rich: Option<Vec<u8>>,
    ) -> Result<(), String> {
        if format.gilding.is_some() != spot.is_some() || format.rich_black != rich.is_some() {
            return Err("Select a registered mask for every requested finishing plate".into());
        }
        let size = format.layout.mechanical().pixels;
        self.spot = spot.map(|data| Stencil::read(data, size)).transpose()?;
        self.rich = rich.map(|data| Stencil::read(data, size)).transpose()?;
        Ok(())
    }

    fn transform(&self) -> Result<Transform<[u16; 3], [u16; 4]>, String> {
        let data: &[u8] = match self.stock {
            Stock::Coated => include_bytes!("../../assets/color/coated-gcr.icc"),
            Stock::Uncoated => include_bytes!("../../assets/color/uncoated-gcr.icc"),
        };
        let link = Profile::new_icc(data).map_err(|e| e.to_string())?;
        Transform::new_multiprofile(
            &[&link],
            PixelFormat::RGB_16,
            PixelFormat::CMYK_16,
            Intent::Perceptual,
            Flags::NO_OPTIMIZE,
        )
        .map_err(|e| e.to_string())
    }
}

// The ICC TIFF specification defines tag 34675 as UNDEFINED, not BYTE.
struct IccTag<'a>(&'a [u8]);
impl TiffValue for IccTag<'_> {
    const BYTE_LEN: u8 = 1;
    const FIELD_TYPE: Type = Type::UNDEFINED;
    fn count(&self) -> usize {
        self.0.len()
    }
    fn data(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.into()
    }
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InkReport {
    pixels: usize,
    max_total_code: u32,
    max_link_code: u32,
    corrected_pixels: usize,
}

impl InkReport {
    // Tetrahedral LUT interpolation and final code rounding can put an edge
    // sample above the press ceiling. Keep K and reduce the three process inks
    // in their current proportions, using integer floors so the bound is exact.
    fn enforce(&mut self, pixel: &mut [u16; 4], cap: u32) {
        self.pixels += 1;
        let sum = pixel.iter().map(|&v| u32::from(v)).sum::<u32>();
        self.max_link_code = self.max_link_code.max(sum);
        if sum <= cap {
            self.max_total_code = self.max_total_code.max(sum);
            return;
        }
        let k = u32::from(pixel[3]);
        let available = u64::from(cap - k);
        let cmy = u64::from(sum - k);
        for ink in &mut pixel[..3] {
            *ink = (u64::from(*ink) * available / cmy) as u16;
        }
        self.corrected_pixels += 1;
        self.max_total_code = self
            .max_total_code
            .max(pixel.iter().map(|&v| u32::from(v)).sum());
    }
}

const STRIP_ROWS: u32 = 16;

fn encode_process(
    file: &mut (impl std::io::Write + std::io::Seek),
    [width, height]: [u32; 2],
    profile: &[u8],
    separated: &[[u16; 4]],
) -> Result<(), String> {
    let mut encoder = TiffEncoder::new(file)
        .map_err(|e| e.to_string())?
        .with_compression(Compression::Lzw)
        .with_predictor(Predictor::Horizontal);
    let mut image = encoder
        .new_image::<CMYK16>(width, height)
        .map_err(|e| e.to_string())?;
    image
        .encoder()
        .write_tag(Tag::IccProfile, IccTag(profile))
        .map_err(|e| e.to_string())?;
    image.resolution(ResolutionUnit::Inch, Rational { n: 300, d: 1 });
    image
        .rows_per_strip(STRIP_ROWS)
        .map_err(|e| e.to_string())?;
    // The TIFF codec enables the configured LZW compressor in write_data.
    image
        .write_data(bytemuck::cast_slice(separated))
        .map_err(|e| e.to_string())
}

fn write_tiff(
    path: &Path,
    size: [u32; 2],
    profile: &PrintProfile,
    pixels: Vec<[f32; 4]>,
    progress: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(InkReport, Vec<u8>), String> {
    let transform = profile.transform()?;
    let width = size[0];
    let cap = profile.stock.tac() * 65535 / 100;
    let mut report = InkReport::default();
    let mut separated = Vec::with_capacity(pixels.len());
    let mut process = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut big_endian = Vec::new();
    let mut rgb = Vec::new();
    let mut cmyk = Vec::new();
    for strip in pixels.chunks((width * STRIP_ROWS) as usize) {
        progress("Separating CMYK and checking ink coverage")?;
        rgb.clear();
        rgb.extend(
            strip
                .iter()
                .map(|p| std::array::from_fn(|i| Encoding::Srgb.quantize(p[i], false))),
        );
        cmyk.resize(strip.len(), [0; 4]);
        transform.transform_pixels(&rgb, &mut cmyk);
        big_endian.clear();
        for pixel in &mut cmyk {
            if profile
                .rich
                .as_ref()
                .is_some_and(|s| s.marks(report.pixels))
            {
                *pixel = [39321, 26214, 26214, 65535];
            }
            report.enforce(pixel, cap);
            for sample in *pixel {
                big_endian.extend_from_slice(&sample.to_be_bytes());
            }
        }
        process.write_all(&big_endian).map_err(|e| e.to_string())?;

        separated.extend_from_slice(&cmyk);
    }
    drop(pixels);
    progress("Compressing 16-bit TIFF strips")?;
    let mut file = BufWriter::new(File::create(path).map_err(|e| e.to_string())?);
    encode_process(&mut file, size, &profile.bytes, &separated)?;
    file.flush().map_err(|e| e.to_string())?;
    Ok((report, process.finish().map_err(|e| e.to_string())?))
}

impl LiveWheel {
    pub async fn export_print_mechanical(
        &mut self,
        parent: &Path,
        times: [f64; 2],
        format: PrintFormat,
        profile: &PrintProfile,
        mut progress: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<PathBuf, String> {
        if !times.iter().all(|v| v.is_finite())
            || profile.stock != format.stock
            || profile.spot.is_some() != format.gilding.is_some()
            || profile.rich.is_some() != format.rich_black
        {
            return Err("Invalid film times or stock/profile mismatch".into());
        }
        let spec = format.layout.mechanical();
        progress("Preparing full-bleed optical projection")?;
        let target = PlateTarget::new(self, layout::tile_size(spec.pixels), Encoding::Srgb).await?;
        let pixels = layout::capture(self, &target, times, spec.pixels, &mut progress)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let dir = parent.join(format!("TSS-print-{}-{stamp}", self.wheel));
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        let result = (|| {
            let (ink, compressed) = write_tiff(
                &dir.join("artwork.tif"),
                spec.pixels,
                profile,
                pixels,
                &mut progress,
            )?;
            progress("Writing registered press PDF")?;
            finishing::write_pdf(&dir.join("press.pdf"), format, profile, &compressed)?;
            fs::write(dir.join("dieline.svg"), spec.guide_svg()).map_err(|e| e.to_string())?;
            let data = serde_json::json!({
                "profile":"PRINT_CMYK", "format":format, "geometry":spec, "canvasMm":spec.canvas_mm(),
                "wheel":self.wheel, "motion":self.motion, "materialMotion":self.material_motion, "filmTimes":times, "visibleFilms":self.slide_visibility,
                "scope":"Selected wheel at full density, before electronic feedback. Flat orthographic full-bleed process art, separate registered finishing guides.",
                "processFile":"artwork.tif", "pressFile":"press.pdf", "spotPlate":format.gilding.map(|g|g.plate()), "colophonTypePt":format.catalog.map(|_|6.5),"colophonBlack":"C0 M0 Y0 K100 overprint on uninked panel","largeSolidBlack":"C60 M40 Y40 K100 from the registered mask", "nonPrintingGuide":"dieline.svg", "physicalResolutionPpi":300,
                "destinationIcc":format.stock.profile_name(), "tacPercent":format.stock.tac(),
                "colorEngine":"Little CMS 2, 16-bit RGB to 16-bit CMYK Argyll device link, perceptual mapping",
                "blackGeneration":"50% of the attainable K locus (Argyll -Kh)", "inkCoverage":ink,
            });
            fs::write(
                dir.join("mechanical.json"),
                serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        })();
        if let Err(e) = result {
            for name in ["artwork.tif", "press.pdf", "dieline.svg", "mechanical.json"] {
                let _ = fs::remove_file(dir.join(name));
            }
            let _ = fs::remove_dir(&dir);
            return Err(e);
        }
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lzw_tiff_decodes_all_cmyk16_codes_across_the_last_partial_strip() {
        let size = [13, 19];
        let pixels: Vec<[u16; 4]> = (0..size[0] * size[1])
            .map(|i| std::array::from_fn(|c| ((i * 16709 + c as u32 * 3571) % 65536) as u16))
            .collect();
        let mut file = std::io::Cursor::new(Vec::new());
        encode_process(&mut file, size, b"test ICC tag", &pixels).unwrap();
        file.set_position(0);
        let mut decoder = tiff::decoder::Decoder::new(file).unwrap();
        let data = decoder.read_image().unwrap();
        let tiff::decoder::DecodingResult::U16(actual) = data else {
            panic!("Expected native 16-bit samples")
        };
        assert_eq!(actual, bytemuck::cast_slice::<[u16; 4], u16>(&pixels));
    }

    #[test]
    fn enforce_tac_without_replacing_k_or_altering_sub_limit_samples() {
        let mut report = InkReport::default();
        for stock in [Stock::Coated, Stock::Uncoated] {
            let cap = stock.tac() * 65535 / 100;
            for p in [
                [65535; 4],
                [61234, 55918, 50004, 65535],
                [125, 665, 10, 100],
                [0, 0, 0, 65535],
            ] {
                let mut v = p;
                report.enforce(&mut v, cap);
                assert_eq!(v[3], p[3]);
                assert!(v.iter().map(|&x| u32::from(x)).sum::<u32>() <= cap);
                if p.iter().map(|&x| u32::from(x)).sum::<u32>() <= cap {
                    assert_eq!(v, p);
                }
            }
        }
    }
}
