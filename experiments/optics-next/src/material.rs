//! Explicit optical-density multipliers for each independently addressed dye.
//! They are scene parameters, not measured dye absorption coefficients.
pub fn dye_strengths(value: Option<&str>) -> Result<[f32; 4], &'static str> {
    let Some(value) = value else { return Ok([1.0; 4]); };
    let mut result = [0.0; 4];
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() != 4 { return Err("TSS_DYE_STRENGTHS needs four comma-separated values"); }
    for (i, part) in parts.iter().enumerate() {
        let n: f32 = part.trim().parse().map_err(|_| "invalid dye strength")?;
        if !n.is_finite() || !(0.0..=100.0).contains(&n) {
            return Err("dye strengths must be finite, between 0 and 100");
        }
        result[i] = n;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::dye_strengths;
    #[test] fn default_is_unit_absorption() { assert_eq!(dye_strengths(None),Ok([1.0;4])); }
    #[test] fn independent_material_parameters() { assert_eq!(dye_strengths(Some("0,0.5,2,3")),Ok([0.0,0.5,2.0,3.0])); }
    #[test] fn rejects_bad_values() {
        for s in ["", "1,2", "1,2,3,NaN", "1,2,3,-1", "1,2,3,101"] { assert!(dye_strengths(Some(s)).is_err()); }
    }
}
