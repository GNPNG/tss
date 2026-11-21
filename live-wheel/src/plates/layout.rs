use super::*;

const MAX_TILE: u32 = 2160;
pub(super) const PANORAMA_OVERLAP: u32 = 216;

// Preserve scale in both axes while filling the output from the native 16:9 gate.
fn crop([w, h]: [u32; 2]) -> [f32; 4] {
    let ratio = (w as f32 / h as f32) / (16. / 9.);
    let sx = ratio.min(1.);
    let sy = (1. / ratio).min(1.);
    [sx, sy, (1. - sx) / 2., (1. - sy) / 2.]
}

pub(super) fn tile_size([w, h]: [u32; 2]) -> [u32; 2] {
    [
        w.div_ceil(w.div_ceil(MAX_TILE)),
        h.div_ceil(h.div_ceil(MAX_TILE)),
    ]
}

/// Evaluate tiles in global gate coordinates. Derivatives have identical scale
/// on both sides of each tile boundary. Padded GPU rows never enter the canvas.
pub(super) fn capture(
    r: &mut LiveWheel,
    target: &PlateTarget,
    times: [f64; 2],
    full: [u32; 2],
    progress: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<Vec<[f32; 4]>, String> {
    let [w, h] = full;
    let [tw, th] = target.size;
    let [sx, sy, ox, oy] = crop(full);
    let mut result = vec![[0.; 4]; w as usize * h as usize];
    for y in (0..h).step_by(th as usize) {
        for x in (0..w).step_by(tw as usize) {
            progress("Rendering full-resolution optical tiles")?;
            let gate = [
                sx * tw as f32 / w as f32,
                sy * th as f32 / h as f32,
                ox + sx * x as f32 / w as f32,
                oy + sy * y as f32 / h as f32,
            ];
            r.queue.write_buffer(
                &target.material,
                std::mem::size_of::<Material>() as u64,
                bytemuck::bytes_of(&gate),
            );
            let tile = target.capture(r, times, r.slide_visibility)?;
            let count = tw.min(w - x) as usize;
            for (dy, row) in tile
                .chunks_exact(tw as usize)
                .take(th.min(h - y) as usize)
                .enumerate()
            {
                let at = (y as usize + dy) * w as usize + x as usize;
                result[at..at + count].copy_from_slice(&row[..count]);
            }
        }
    }
    Ok(result)
}

// The first band crossfades a genuine overscan exposure in linear light. This
// makes the end-to-start transition continuous as well as the four panel joins.
fn wrap_panorama(mut pixels: Vec<[f32; 4]>, size: [u32; 2]) -> Vec<[f32; 4]> {
    let [width, height] = size.map(|v| v as usize);
    let overlap = PANORAMA_OVERLAP as usize;
    let stride = width + overlap;
    for y in 0..height {
        let start = y * stride;
        for x in 0..overlap {
            let t = (x as f32 + 0.5) / overlap as f32;
            let blend = t * t * (3. - 2. * t);
            let a = pixels[start + width + x];
            let b = pixels[start + x];
            pixels[start + x] = std::array::from_fn(|c| a[c] + blend * (b[c] - a[c]));
        }
        pixels.copy_within(start..start + width, y * width);
    }
    pixels.truncate(width * height);
    pixels
}

pub(super) fn source_size(format: DigitalFormat) -> [u32; 2] {
    let [w, h] = format.size();
    [
        w + if format.layout == Layout::Panorama {
            PANORAMA_OVERLAP
        } else {
            0
        },
        h,
    ]
}

pub(super) fn write_master(
    r: &mut LiveWheel,
    dir: &Path,
    target: &PlateTarget,
    times: [f64; 2],
    format: DigitalFormat,
    progress: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let size = format.size();
    let mut pixels = capture(r, target, times, source_size(format), progress)?;
    if format.layout == Layout::Panorama {
        pixels = wrap_panorama(pixels, size);
    }
    write_png(
        &dir.join("projection.png"),
        size,
        format.encoding,
        progress,
        pixels.iter().copied(),
    )?;
    if format.layout == Layout::Panorama {
        for n in 0..5 {
            let name = format!("EPNI-{:03}.png", 6 + n);
            write_png(
                &dir.join(name),
                [2160, 2160],
                format.encoding,
                progress,
                pixels
                    .chunks_exact(size[0] as usize)
                    .flat_map(|row| row[n * 2160..(n + 1) * 2160].iter().copied()),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn square_and_vertical_preserve_the_optical_aspect() {
        for size in [[4000, 4000], [2160, 3840], [11016, 2160], [839, 3780]] {
            let [sx, sy, ox, oy] = crop(size);
            assert!((sx * 16. / 9. / sy - size[0] as f32 / size[1] as f32).abs() < 1e-5);
            assert_eq!((sx + 2. * ox, sy + 2. * oy), (1., 1.));
        }
    }
}
