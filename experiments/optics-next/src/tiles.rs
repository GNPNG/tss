//! Row-bounded dispatch planning. This limits work submitted per dispatch; it
//! does not establish a device-specific safe workload or prevent GPU faults.
use std::ops::Range;

pub fn plan(height: u32, requested: Option<u32>) -> Result<Vec<Range<u32>>, &'static str> {
    if height == 0 { return Err("height must be positive"); }
    let rows = match requested {
        Some(n) if (8..=4096).contains(&n) && n % 8 == 0 => n,
        Some(_) => return Err("TSS_TILE_ROWS must be a multiple of 8 between 8 and 4096"),
        None => return Ok(vec![0..height]),
    };
    let mut ranges = Vec::new();
    let mut start = 0u32;
    while start < height {
        let end = start.saturating_add(rows).min(height);
        ranges.push(start..end);
        start = end;
    }
    Ok(ranges)
}

#[cfg(test)]
mod tests {
    use super::plan;
    #[test] fn default_one_dispatch() { assert_eq!(plan(17,None).unwrap(),vec![0..17]); }
    #[test] fn boundaries_cover_partial_final_tile() {
        assert_eq!(plan(19,Some(8)).unwrap(),vec![0..8,8..16,16..19]);
        assert_eq!(plan(16,Some(16)).unwrap(),vec![0..16]);
        assert_eq!(plan(7,Some(16)).unwrap(),vec![0..7]);
    }
    #[test] fn reject_invalid_sizes() {
        for n in [0,1,7,9,4097,4104] { assert!(plan(20,Some(n)).is_err()); }
        assert!(plan(0,None).is_err());
    }
}
