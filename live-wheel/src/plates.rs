//! The selected wheel's optical image before electronic feedback or display encoding.
use super::*;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

mod color;
mod guides;
mod layout;
mod rec709;
mod print;
pub use print::PrintProfile;
use crate::production::{DigitalFormat, Encoding, Layout};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PlateMaterial {
    optics: Material,
    gate: [f32; 4],
}

const FORMAT: gpu::TextureFormat = gpu::TextureFormat::Rgba32Float;

pub(super) struct PlateTarget {
    pub target: Target,
    pub optics: [Pass; 4],
    pub material: gpu::Buffer,
    readback: gpu::Buffer,
    row_pitch: u32,
    encoding: Encoding,
    size: [u32; 2],
}

impl PlateTarget {
    async fn new(r: &LiveWheel, size: [u32; 2], encoding: Encoding) -> Result<Self, String> {
        let [w, h] = size;
        check_extent(size, r.device.limits().max_texture_dimension_2d)?;
        let row_pitch = (w * 16).div_ceil(gpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * gpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes = u64::from(row_pitch) * u64::from(h);
        if w == 0 || h == 0 || bytes > r.device.limits().max_buffer_size {
            return Err("GPU cannot read back the requested RGBA32F plate".into());
        }
        r.device.push_error_scope(gpu::ErrorFilter::Validation);
        r.device.push_error_scope(gpu::ErrorFilter::OutOfMemory);
        let target = Target::new(&r.device, "32-bit linear wheel projection", size, FORMAT);
        let vertex = shader(&r.device, "Plate quad", include_str!("plates/vertex.wgsl"));
        let optics = feedback::linear_optics(&r.device, &vertex, FORMAT);
        let mut material = r.materials[&r.wheel];
        material.lens[0] = r.focus.unwrap_or(-1.);
        material.lens[1] = 1.; // Shared optics return linear radiance.
        let material = uniform(
            &r.device,
            "Linear plate material",
            &PlateMaterial {
                optics: material,
                gate: [1., 1., 0., 0.],
            },
        );
        let readback = r.device.create_buffer(&gpu::BufferDescriptor {
            label: Some("32-bit optical plate readback"),
            size: bytes,
            usage: gpu::BufferUsages::COPY_DST | gpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let memory = r.device.pop_error_scope().await;
        let validation = r.device.pop_error_scope().await;
        if let Some(error) = memory.or(validation) {
            return Err(format!(
                "Could not allocate linear 32-bit optical plates: {error}"
            ));
        }
        Ok(Self {
            target,
            optics,
            material,
            readback,
            row_pitch,
            encoding,
            size,
        })
    }

    fn capture(
        &self,
        r: &mut LiveWheel,
        times: [f64; 2],
        visibility: [bool; 2],
    ) -> Result<Vec<[f32; 4]>, String> {
        r.render_one(
            None,
            times[0],
            times[1],
            0.,
            Some(OpticalExposure::Plate(self, visibility)),
        );
        let mut enc = r.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            self.target.texture.as_image_copy(),
            gpu::ImageCopyBuffer {
                buffer: &self.readback,
                layout: gpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row_pitch),
                    rows_per_image: Some(self.size[1]),
                },
            },
            extent(self.size, 1),
        );
        r.queue.submit(Some(enc.finish()));
        let slice = self.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(gpu::MapMode::Read, move |status| {
            let _ = tx.send(status);
        });
        movie::wait_for_callback(&r.device, rx)?.map_err(|e| e.to_string())?;
        let mut image = Vec::with_capacity(self.size[0] as usize * self.size[1] as usize);
        {
            let mapping = slice.get_mapped_range();
            for row in mapping.chunks_exact(self.row_pitch as usize) {
                let pixels: &[[f32; 4]] = bytemuck::cast_slice(&row[..self.size[0] as usize * 16]);
                if self.encoding == Encoding::DisplayP3 {
                    image.extend(pixels.iter().map(|&p| self.encoding.working_rgb(p)));
                } else {
                    image.extend_from_slice(pixels);
                }
            }
        }
        self.readback.unmap();
        let image: Vec<[f32; 4]> = image;
        if !image
            .iter()
            .all(|pixel| pixel.iter().all(|v| v.is_finite()) && pixel[3] == 1.)
        {
            return Err("Optics supplied a non-finite or non-opaque exposure".into());
        }
        Ok(image)
    }
}

// In linear display RGB, find the smallest common alpha for which every channel
// of the straight-alpha foreground is representable: target = base*(1-a) + fg*a.
fn over_plate(base: [f32; 4], target: [f32; 4]) -> [f32; 4] {
    let base = base.map(|v| v.clamp(0., 1.));
    let target = target.map(|v| v.clamp(0., 1.));
    let mut alpha = 0f32;
    for (&b, &t) in base[..3].iter().zip(&target[..3]) {
        if t > b {
            alpha = alpha.max((t - b) / (1. - b));
        } else if t < b {
            alpha = alpha.max((b - t) / b);
        }
    }
    if alpha == 0. {
        return [0.; 4];
    }
    let mut fg = [alpha; 4];
    for i in 0..3 {
        fg[i] = (base[i] + (target[i] - base[i]) / alpha).clamp(0., 1.);
    }
    fg
}

fn write_png(
    path: &Path,
    size: [u32; 2],
    encoding: Encoding,
    progress: &mut impl FnMut(&str) -> Result<(), String>,
    pixels: impl Iterator<Item = [f32; 4]>,
) -> Result<(), String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid plate filename")?;
    progress(name)?;
    let file = File::create_new(path).map_err(|e| e.to_string())?;
    let mut out = BufWriter::new(file);
    let mut info = png::Info::with_size(size[0], size[1]);
    info.icc_profile = Some(std::borrow::Cow::Borrowed(encoding.icc()));
    let mut encoder = png::Encoder::with_info(&mut out, info).map_err(|e| e.to_string())?;
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Sixteen);
    encoder.set_compression(png::Compression::Fast);
    let mut png = encoder.write_header().map_err(|e| e.to_string())?;
    png.write_chunk(png::chunk::cICP, &encoding.cicp())
        .map_err(|e| e.to_string())?;
    let mut stream = png
        .stream_writer_with_size(64 * 1024)
        .map_err(|e| e.to_string())?;
    let mut row = Vec::with_capacity(size[0] as usize * 8);
    let mut rows_written = 0;
    for pixel in pixels {
        for (ch, v) in pixel.into_iter().enumerate() {
            row.extend_from_slice(&encoding.quantize(v, ch == 3).to_be_bytes());
        }
        if row.len() == size[0] as usize * 8 {
            stream.write_all(&row).map_err(|e| e.to_string())?;
            row.clear();
            rows_written += 1;
            if rows_written % 64 == 0 {
                progress(name)?;
            }
        }
    }
    stream.finish().map_err(|e| e.to_string())?;
    png.finish().map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())
}

impl LiveWheel {
    pub async fn plate_framing_preview(&mut self, times: [f64; 2]) -> Result<Vec<u8>, String> {
        let size = [512, 288];
        let target = PlateTarget::new(self, size, Encoding::Srgb).await?;
        let image = target.capture(self, times, self.slide_visibility)?;
        let data: Vec<u8> = image
            .into_iter()
            .flat_map(|p| {
                p.into_iter().enumerate().map(|(c, v)| {
                    ((u32::from(Encoding::Srgb.quantize(v, c == 3)) + 128) / 257) as u8
                })
            })
            .collect();
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, size[0], size[1]);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::RelativeColorimetric);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&data).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        Ok(bytes)
    }
    /// Capture the selected wheel into a new directory under the operator's chosen folder.
    pub async fn export_4k_plates(
        &mut self,
        parent: &Path,
        times: [f64; 2],
        mut progress: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<PathBuf, String> {
        self.export_digital_plates(parent, times, DigitalFormat::default(), &mut progress)
            .await
    }

    pub async fn export_digital_plates(
        &mut self,
        parent: &Path,
        times: [f64; 2],
        format: DigitalFormat,
        mut progress: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<PathBuf, String> {
        format.validate()?;
        let size = if format.layout == Layout::Visualizer {
            format.size()
        } else {
            layout::tile_size(layout::source_size(format))
        };
        progress("Preparing optical projection")?;
        if !times.iter().all(|v| v.is_finite()) {
            return Err("Invalid plate film times".into());
        }
        let target = PlateTarget::new(self, size, format.encoding).await?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let dir = parent.join(format!("TSS-{}-{stamp}", self.wheel));
        fs::create_dir(&dir).map_err(|e| format!("Could not create the new plate folder: {e}"))?;
        let result = (|| {
            if format.layout == Layout::Visualizer {
                self.write_plate_set(&dir, &target, times, &mut progress)?;
            } else {
                layout::write_master(self, &dir, &target, times, format, &mut progress)?;
            }
            guides::write_guides(&dir, format)?;
            self.plate_manifest(&dir, times, format)
        })();
        if let Err(error) = result {
            // Only the known members of this exclusively created bundle belong to us.
            for name in [
                "BG.png",
                "MID.png",
                "FG.png",
                "projection.png",
                "rear-solo.png",
                "front-solo.png",
                "plates.json",
                "guides.svg",
                "EPNI-006.png",
                "EPNI-007.png",
                "EPNI-008.png",
                "EPNI-009.png",
                "EPNI-010.png",
            ] {
                let _ = fs::remove_file(dir.join(name));
            }
            let _ = fs::remove_dir(&dir);
            return Err(error);
        }
        Ok(dir)
    }

    fn write_plate_set(
        &mut self,
        dir: &Path,
        target: &PlateTarget,
        times: [f64; 2],
        progress: &mut impl FnMut(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        let size = target.size;
        let [rear, front] = self.slide_visibility;
        let mut base = target.capture(self, times, [false, false])?;
        write_png(
            &dir.join("BG.png"),
            size,
            target.encoding,
            progress,
            base.iter().copied(),
        )?;
        let rear_solo = target.capture(self, times, [true, false])?;
        write_png(
            &dir.join("rear-solo.png"),
            size,
            target.encoding,
            progress,
            rear_solo.iter().copied(),
        )?;
        write_png(
            &dir.join("MID.png"),
            size,
            target.encoding,
            progress,
            base.iter().zip(&rear_solo).map(
                |(&b, &r)| {
                    if rear {
                        over_plate(b, r)
                    } else {
                        [0.; 4]
                    }
                },
            ),
        )?;
        if rear {
            base = rear_solo;
        } else {
            drop(rear_solo);
        }
        let front_solo = target.capture(self, times, [false, true])?;
        write_png(
            &dir.join("front-solo.png"),
            size,
            target.encoding,
            progress,
            front_solo.iter().copied(),
        )?;
        let combined;
        let projection = if rear && front {
            drop(front_solo);
            combined = target.capture(self, times, [true, true])?;
            &combined
        } else if front {
            &front_solo
        } else {
            &base
        };
        write_png(
            &dir.join("projection.png"),
            size,
            target.encoding,
            progress,
            projection.iter().copied(),
        )?;
        write_png(
            &dir.join("FG.png"),
            size,
            target.encoding,
            progress,
            base.iter().zip(projection).map(|(&b, &p)| over_plate(b, p)),
        )?;
        Ok(())
    }
    fn plate_manifest(
        &self,
        dir: &Path,
        times: [f64; 2],
        format: DigitalFormat,
    ) -> Result<(), String> {
        let [rear, front] = self.slide_visibility;
        let mut manifest = serde_json::json!({
            "schemaVersion": 1, "profile": "DIGITAL_4K", "size": format.size(),
            "layout": format.layout, "horizonY": format.horizon_y,
            "scope": "Selected wheel projected alone at full optical density, before stack propagation and electronic feedback.",
            "wheel": self.wheel, "layerId": self.active, "motion": self.motion, "materialMotion": self.material_motion, "focus": self.focus,
            "rearSeconds": times[0], "frontSeconds": times[1], "visibleFilms": {"rear": rear, "front": front},
            "gpu": self.adapter_name(), "source": "RGBA32Float linear optical radiance before video feedback",
            "encoding":format.encoding, "cICP":format.encoding.cicp(), "iccEmbedded":true, "alpha":"16-bit, straight linear",
            "projection": "projection.png",
            "isolatedExposures": {"rear": "rear-solo.png", "front": "front-solo.png"},
            "layersBackToFront": ["BG.png", "MID.png", "FG.png"],
            "compositing": "Decode RGB to linear light. Use straight-alpha Normal/Over, at original alignment. BG is clear carrier under the lamp. MID takes BG to the visible rear stage. FG takes that result to the selected projection, including front-induced refraction, common-pupil sampling and flare. The alpha encodes the minimum required blend for this registered backing. Moving a plate changes the image; recapture after changing the optical geometry. Each solo is an opaque exposure with the original lamp. The two solos alone cannot reconstruct the overlap.",
        });
        if format.layout != Layout::Visualizer {
            for key in ["layersBackToFront", "isolatedExposures", "compositing"] {
                manifest.as_object_mut().unwrap().remove(key);
            }
            manifest["framing"] =
                serde_json::json!("Centered aspect-preserving crop of the selected optical gate");
        }
        if format.layout == Layout::Panorama {
            manifest["panels"] = serde_json::json!([
                "EPNI-006.png",
                "EPNI-007.png",
                "EPNI-008.png",
                "EPNI-009.png",
                "EPNI-010.png"
            ]);
            manifest["loopOverlapPixels"] = serde_json::json!(layout::PANORAMA_OVERLAP);
        }
        let mut file =
            BufWriter::new(File::create_new(dir.join("plates.json")).map_err(|e| e.to_string())?);
        serde_json::to_writer_pretty(&mut file, &manifest).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn straight_over_reconstructs_darkening_brightening_and_saturated_channels() {
        for (base, goal) in [
            ([0., 0.5, 1., 1.], [0., 0.5, 1., 1.]),
            ([0., 0.5, 1., 1.], [1., 0.4, 0., 1.]),
            ([0.51, 0.99, 0.02, 1.], [0.1, 0.995, 0.01, 1.]),
            ([1.01, 0.2, -0.1, 1.], [0.85, 0.3, 0.04, 1.]),
        ] {
            let layer = over_plate(base, goal);
            assert!(layer.iter().all(|v| (0.0..=1.).contains(v)));
            for c in 0..3 {
                let blend = base[c].clamp(0., 1.) * (1. - layer[3]) + layer[c] * layer[3];
                assert!((blend - goal[c].clamp(0., 1.)).abs() < 2e-7);
            }
        }
    }
}

#[cfg(test)]
pub(super) fn check_preview_alignment(
    r: &mut LiveWheel,
    times: [f64; 2],
    preview: &[u8],
) -> Result<(), String> {
    let target = pollster::block_on(PlateTarget::new(r, r.preview_size(), Encoding::Rec709))?;
    let linear = target.capture(r, times, [true; 2])?;
    if linear.len() * 4 != preview.len() {
        return Err("Preview dimensions differ".into());
    }
    let max = linear
        .iter()
        .zip(preview.chunks_exact(4))
        .flat_map(|(px, reference)| {
            (0..3).map(move |i| {
                let v = px[i].clamp(0., 1.);
                let encoded = if v <= 0.0031308 {
                    12.92 * v
                } else {
                    1.055 * v.powf(1. / 2.4) - 0.055
                };
                ((255. * encoded).round() as u8).abs_diff(reference[i])
            })
        })
        .max()
        .unwrap();
    println!("Maximum per-channel difference from native preview after sRGB encoding: {max}/255");
    if max > 1 {
        return Err("Linear plate and native preview orientation or optics differ".into());
    }
    Ok(())
}
