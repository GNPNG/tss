//! Projector white point in linear Rec.709, relative to the calibrated 6500 K setup.

pub(super) const REFERENCE_K: u32 = 6500;

/// Planckian locus in CIE 1960 UCS. Krystek (1985), equations 14–15,
/// valid over 1000–15000 K: https://doi.org/10.1002/col.5080100109
fn planck_uv(kelvin: u32) -> [f64; 2] {
    let t = f64::from(kelvin);
    let u = (0.860117757 + t * (1.54118254e-4 + t * 1.28641212e-7))
        / (1. + t * (8.42420235e-4 + t * 7.08145163e-7));
    let v = (0.317398726 + t * (4.22806245e-5 + t * 4.20481691e-8))
        / (1. + t * (-2.89741816e-5 + t * 1.61456053e-7));
    [u, v]
}

fn blackbody_rgb(kelvin: u32) -> [f64; 3] {
    let [u, v] = planck_uv(kelvin);
    // u = 4X/(X+15Y+3Z), v = 6Y/(X+15Y+3Z), with Y = 1.
    let x = 1.5 * u / v;
    let z = (6. / v - x - 15.) / 3.;
    // D65 XYZ to linear sRGB/Rec.709 (W3C CSS Color 4, colour conversion code).
    [
        3.240969942 * x - 1.537383178 - 0.498610760 * z,
        -0.969243636 * x + 1.875967502 + 0.041555057 * z,
        0.055630080 * x - 0.203976959 + 1.056971514 * z,
    ]
}

pub(super) fn linear_gain(kelvin: u32) -> Result<[f32; 4], String> {
    if !(2700..=10000).contains(&kelvin) {
        return Err("Projector lamp temperature must be between 2700 and 10000 K".into());
    }
    let reference = blackbody_rgb(REFERENCE_K);
    let lamp = blackbody_rgb(kelvin);
    let [r, g, b] = std::array::from_fn(|i| lamp[i] / reference[i]);
    // The operator controls chromaticity independently of exposure.
    let y = 0.2126390059 * r + 0.7151686788 * g + 0.0721923153 * b;
    Ok([(r / y) as f32, (g / y) as f32, (b / y) as f32, 1.])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projector_white_tracks_cie_data_with_constant_luminance() {
        // Independent integration of Planck spectral radiance at 1 nm with
        // CIE 1931 2-degree functions: https://doi.org/10.25039/CIE.DS.xvudnb9b
        for (k, reference) in [
            (2700, [0.262496, 0.351567]),
            (2856, [0.255951, 0.349520]),
            (4500, [0.217311, 0.328457]),
            (6500, [0.200448, 0.310361]),
            (10000, [0.190319, 0.293264]),
        ] {
            for (actual, expected) in planck_uv(k).into_iter().zip(reference) {
                assert!((actual - expected).abs() < 0.00015, "CIE 1960 UCS at {k} K");
            }
            let [r, g, b, _] = linear_gain(k).unwrap();
            let luminance = 0.212639 * r + 0.715169 * g + 0.072192 * b;
            assert!((luminance - 1.).abs() < 1e-6);
        }
        assert_eq!(linear_gain(REFERENCE_K).unwrap(), [1.; 4]);
        let warm = linear_gain(2700).unwrap();
        let cool = linear_gain(10000).unwrap();
        assert!(warm[0] > 1. && warm[2] < 1. && cool[0] < 1. && cool[2] > 1.);
        assert!(linear_gain(2699).is_err() && linear_gain(10001).is_err());
    }
}
