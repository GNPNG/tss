fn phase(rho_r: f32, rho_b: f32) -> f32 {
    include!(env!("TAICHI_PHASE_EXPR"))
}

#[test]
fn pure_red() {
    for density in [0.125, 0.5, 1.0, 2.0, 16.0] {
        assert_eq!(phase(density, 0.0), 1.0);
    }
}

#[test]
fn pure_blue() {
    for density in [0.125, 0.5, 1.0, 2.0, 16.0] {
        assert_eq!(phase(0.0, density), -1.0);
    }
}

#[test]
fn equal_component_densities() {
    for density in [0.125, 0.5, 1.0, 2.0, 16.0] {
        assert_eq!(phase(density, density), 0.0);
    }
}

#[test]
fn normalized_contrast_at_nonunit_density() {
    let phi = phase(0.8, 0.4);
    assert!((phi - 1.0 / 3.0).abs() <= f32::EPSILON, "phi = {phi}");
    assert_eq!(phase(0.4, 0.8), -phi);
}

#[test]
fn scaling_both_components_preserves_contrast() {
    let phi = phase(3.0, 5.0);
    for exponent in -32..=32 {
        let scale = 2.0_f32.powi(exponent);
        assert_eq!(phase(3.0 * scale, 5.0 * scale), phi);
    }
}

#[test]
fn unit_total_initialization_round_trips() {
    for psi in [-1.0, -0.75, 0.0, 0.25, 1.0] {
        let red = (psi + 1.0) / 2.0;
        assert_eq!(phase(red, 1.0 - red), psi);
    }
}

#[test]
fn zero_total_has_no_defined_contrast() {
    assert!(phase(0.0, 0.0).is_nan());
}
