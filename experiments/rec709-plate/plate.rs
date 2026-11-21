//! Conversion for high-precision Rec.709 plates.
//!
//! The source is straight-alpha RGBA in linear light with Rec.709 primaries.
//! This module converts samples only; it does not create or tag a PNG, check
//! framebuffer capabilities, or obtain pixels from the renderer.

use std::io::{self, Write};
#[path = "../../live-wheel/src/plates/rec709.rs"]
mod rec709;
use rec709::quantize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowOrigin {
    TopLeft,
    BottomLeft,
}

/// Write top-down, interleaved, little-endian RGBA16 samples.
///
/// RGB receives the Rec.709 opto-electronic transfer function. Alpha stays
/// linear and unassociated. Finite out-of-range samples are clipped; no tone
/// mapping, gamut conversion, or unpremultiplication is performed. Input size
/// and all samples are checked before any output is written.
pub fn write_rec709_rgba16le<W: Write>(
    input: &[f32],
    width: u32,
    height: u32,
    origin: RowOrigin,
    output: &mut W,
) -> io::Result<()> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidInput, message);
    if width == 0 || height == 0 {
        return Err(invalid("plate dimensions must be nonzero"));
    }
    let row_samples = (width as usize)
        .checked_mul(4)
        .ok_or_else(|| invalid("plate dimensions overflow"))?;
    let samples = row_samples
        .checked_mul(height as usize)
        .ok_or_else(|| invalid("plate dimensions overflow"))?;
    if input.len() != samples {
        return Err(invalid("plate sample count does not match dimensions"));
    }
    if input.iter().any(|sample| !sample.is_finite()) {
        return Err(invalid("plate contains non-finite samples"));
    }
    let row_bytes = row_samples
        .checked_mul(2)
        .ok_or_else(|| invalid("plate dimensions overflow"))?;
    let mut row = Vec::with_capacity(row_bytes);
    for y in 0..height as usize {
        let source_y = match origin {
            RowOrigin::TopLeft => y,
            RowOrigin::BottomLeft => height as usize - 1 - y,
        };
        row.clear();
        let start = source_y * row_samples;
        for (channel, sample) in input[start..start + row_samples].iter().enumerate() {
            let quantized = quantize(*sample, channel % 4 == 3);
            row.extend_from_slice(&quantized.to_le_bytes());
        }
        output.write_all(&row)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(bytes: &[u8]) -> Vec<u16> {
        bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect()
    }

    #[test]
    fn transfer_and_straight_alpha_are_independent() {
        let input = [0.5, 0.25, 0.01, 0.0, 0.5, 0.25, 0.01, 1.0];
        let mut output = Vec::new();
        write_rec709_rgba16le(&input, 2, 1, RowOrigin::TopLeft, &mut output).unwrap();
        let output = samples(&output);
        assert_eq!(output, [46236, 32108, 2949, 0, 46236, 32108, 2949, 65535]);
    }

    #[test]
    fn bottom_left_origin_reverses_rows() {
        let input = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let mut output = Vec::new();
        write_rec709_rgba16le(&input, 1, 2, RowOrigin::BottomLeft, &mut output).unwrap();
        assert_eq!(samples(&output), [65535, 65535, 65535, 65535, 0, 0, 0, 0]);
    }

    #[test]
    fn clips_finite_values_and_preserves_more_than_eight_bits() {
        let mut input = Vec::new();
        for i in 0..1024 {
            let value = i as f32 / 1023.0;
            input.extend_from_slice(&[value, -1.0, 2.0, value]);
        }
        let mut output = Vec::new();
        write_rec709_rgba16le(&input, 1024, 1, RowOrigin::TopLeft, &mut output).unwrap();
        let pixels = samples(&output);
        assert!(pixels
            .chunks_exact(4)
            .all(|pixel| pixel[1] == 0 && pixel[2] == 65535));
        assert_eq!(
            pixels
                .chunks_exact(4)
                .map(|pixel| pixel[0])
                .collect::<std::collections::HashSet<_>>()
                .len(),
            1024
        );
        assert_eq!(
            pixels
                .chunks_exact(4)
                .map(|pixel| pixel[3])
                .collect::<std::collections::HashSet<_>>()
                .len(),
            1024
        );
    }

    #[test]
    fn rejects_invalid_input_before_writing() {
        for (input, width, height) in [
            (vec![], 0, 1),
            (vec![0.0; 3], 1, 1),
            (vec![0.0, 0.0, 0.0, f32::NAN], 1, 1),
            (vec![0.0, 0.0, f32::INFINITY, 1.0], 1, 1),
        ] {
            let mut output = Vec::new();
            assert_eq!(
                write_rec709_rgba16le(&input, width, height, RowOrigin::TopLeft, &mut output)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert!(output.is_empty());
        }
    }
}
