use std::io::Read;

const HEADER: usize = 24;
const WORLD_HEADER: usize = 40;
const SIZE_LIMIT: usize = 128 * 1024 * 1024;
pub fn bundled_fields() -> Result<Vec<u8>, String> {
    unpack(include_bytes!("../assets/slide-world.rg8.gz"))
}
#[cfg(test)]
pub(crate) fn legacy_fields() -> Result<Vec<u8>, String> {
    unpack(include_bytes!("../assets/slide-fields.rg8.gz"))
}
fn unpack(gz: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(gz)
        .take((SIZE_LIMIT + WORLD_HEADER + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Basilisk field cache: {e}"))?;
    parse(&bytes)?;
    Ok(bytes)
}
pub(crate) struct Fields {
    pub size: [u32; 2],
    pub count: u32,
    pub rate: u32,
    pub data: Vec<u8>,
    view: [u32; 4],
}
struct Layout {
    dimensions: [u32; 4],
    view: [u32; 4],
    header: usize,
}
fn parse(b: &[u8]) -> Result<Layout, String> {
    if b.len() < HEADER || b.len() > SIZE_LIMIT + WORLD_HEADER {
        return Err("Invalid Basilisk field cache header".into());
    }
    let header = match &b[..8] {
        b"TSSDF001" => HEADER,
        b"TSSDF002" if b.len() >= WORLD_HEADER => WORLD_HEADER,
        _ => return Err("Unrecognized or truncated Basilisk field cache".into()),
    };
    let d: [u32; 4] =
        std::array::from_fn(|i| u32::from_le_bytes(b[8 + i * 4..12 + i * 4].try_into().unwrap()));
    let [w, h, n, hz] = d;
    let payload = (w as usize)
        .checked_mul(h as usize)
        .and_then(|s| s.checked_mul(n as usize))
        .and_then(|s| s.checked_mul(2));
    if payload != Some(b.len() - header)
        || b.len() - header > SIZE_LIMIT
        || w == 0
        || h == 0
        || w > 4096
        || h > 4096
        || n < 2
        || !(1..=240).contains(&hz)
    {
        return Err("Invalid independent field dimensions, rate or byte count".into());
    }
    let view: [u32; 4] = if header == WORLD_HEADER {
        std::array::from_fn(|i| u32::from_le_bytes(b[24 + i * 4..28 + i * 4].try_into().unwrap()))
    } else {
        [0, 0, w, h]
    };
    let [left, top, width, height] = view;
    if width == 0 || height == 0 || left > w || top > h || width > w - left || height > h - top {
        return Err("Projection gate outside the stored slide".into());
    }
    Ok(Layout {
        dimensions: d,
        view,
        header,
    })
}
impl Fields {
    pub fn matches_small_front(&self) -> bool {
        self.size == [704, 704]
            && self.view == [96, 208, 512, 288]
            && self.count == 96
            && self.rate == 24
    }
    pub fn coral_release() -> Result<Self, String> {
        let mut bytes = unpack(include_bytes!("../assets/coral-release.rg8.gz"))?;
        let layout = parse(&bytes)?;
        bytes.drain(..layout.header);
        Ok(Self::from_layout(layout, bytes))
    }
    pub fn same_glass_as(&self, other: &Self) -> bool {
        self.size == other.size && self.view == other.view
    }
    pub fn from_slice(bytes: &[u8]) -> Result<Self, String> {
        let layout = parse(bytes)?;
        let data = bytes[layout.header..].to_vec();
        Ok(Self::from_layout(layout, data))
    }
    fn from_layout(layout: Layout, data: Vec<u8>) -> Self {
        let [w, h, count, rate] = layout.dimensions;
        Self {
            size: [w, h],
            count,
            rate,
            data,
            view: layout.view,
        }
    }
    pub fn view_size(&self) -> [u32; 2] {
        [self.view[2], self.view[3]]
    }
    pub fn view_to_cache(&self) -> [f32; 4] {
        let [w, h] = self.size.map(|v| v as f32);
        let [x, y, vw, vh] = self.view.map(|v| v as f32);
        [vw / w, vh / h, x / w, y / h]
    }
    pub fn frame(&self, i: u32) -> &[u8] {
        let stride = (self.size[0] * self.size[1] * 2) as usize;
        &self.data[i as usize * stride..(i as usize + 1) * stride]
    }
    pub fn sample(&self, t: f64) -> (u32, u32, f32) {
        let end = (self.count - 1) as f64;
        let pos = (t.max(0.0) * self.rate as f64) % (2.0 * end);
        let x = if pos > end { 2.0 * end - pos } else { pos };
        // A recorded, nonperiodic solve retraces its path in the live preview.
        // Ease the last third of a second at each end to zero velocity. The
        // cubic Hermite segment meets normal-speed playback with unit slope.
        let width = (self.rate as f64 / 3.0).min(end / 2.0);
        let distance = x.min(end - x);
        let x = if distance < width {
            let u = distance / width;
            let eased = distance * u * (2.0 - u);
            if x < end / 2.0 {
                eased
            } else {
                end - eased
            }
        } else {
            x
        };
        let a = x.floor() as u32;
        (a, (a + 1).min(self.count - 1), (x - a as f64) as f32)
    }
    pub fn sample_motion(&self, motion: crate::MotionPreset, seconds: f64) -> (u32, u32, f32) {
        match motion {
            crate::MotionPreset::WheelFlow | crate::MotionPreset::WheelWithCoralBeads => {
                self.sample(seconds)
            }
            crate::MotionPreset::CoralRelease => {
                let end = self.count - 1;
                let pos = (seconds * f64::from(self.rate)).clamp(0., f64::from(end));
                let first = pos.floor() as u32;
                (first, (first + 1).min(end), (pos - f64::from(first)) as f32)
            }
        }
    }
}

#[cfg(test)]
mod world_tests {
    use super::*;
    #[test]
    fn coral_advances_forward_and_holds_its_detached_geometry() {
        let started = std::time::Instant::now();
        let coral = Fields::coral_release().unwrap();
        println!(
            "First coral cache decode: {:.3} ms",
            started.elapsed().as_secs_f64() * 1000.
        );
        let mut wheel = Fields::from_slice(&bundled_fields().unwrap()).unwrap();
        assert!(coral.same_glass_as(&wheel));
        assert!(coral.frame(0) != wheel.frame(0));
        let end = coral.count - 1;
        let hz = f64::from(coral.rate);
        let sample = |s| coral.sample_motion(crate::MotionPreset::CoralRelease, s);
        assert_eq!(sample(-2.), (0, 1, 0.));
        assert_eq!(sample(1.5 / hz), (1, 2, 0.5));
        assert_eq!(sample(f64::from(end) / hz), (end, end, 0.));
        assert_eq!(sample(10000.), (end, end, 0.));
        wheel.view[2] -= 1;
        assert!(!coral.same_glass_as(&wheel));
    }
    #[test]
    fn bundled_world_keeps_registered_gate_and_offscreen_phases() {
        let raw = bundled_fields().unwrap();
        assert_eq!(&raw[..8], b"TSSDF002");
        let f = Fields::from_slice(&raw).unwrap();
        assert_eq!(
            (f.size, f.view, f.count, f.rate),
            ([704, 704], [96, 208, 512, 288], 96, 24)
        );
        for (code, wanted) in f.view_to_cache().iter().zip([512., 288., 96., 208.]) {
            assert!((code * 704. - wanted).abs() < 0.0001);
        }
        for frame in [0, 30, 95] {
            let mut dye = [0; 2];
            let mut carrier = [0; 2];
            for (i, p) in f.frame(frame).chunks_exact(2).enumerate() {
                let (x, y) = (i % 704, i / 704);
                if !(96..608).contains(&x) || !(208..496).contains(&y) {
                    for c in 0..2 {
                        if p[c] >= 128 {
                            dye[c] += 1;
                        } else {
                            carrier[c] += 1;
                        }
                    }
                }
            }
            assert!(dye.into_iter().all(|n| n > 0) && carrier.into_iter().all(|n| n > 0));
        }
    }
    #[test]
    fn world_header_rejects_outside_empty_overflowing_and_truncated_gates() {
        let make = |view: [u32; 4]| {
            let mut b = b"TSSDF002".to_vec();
            for v in [8u32, 8, 2, 24].into_iter().chain(view) {
                b.extend(v.to_le_bytes());
            }
            b.resize(WORLD_HEADER + 8 * 8 * 2 * 2, 127);
            b
        };
        assert_eq!(
            Fields::from_slice(&make([2, 3, 4, 2]))
                .unwrap()
                .view_to_cache(),
            [0.5, 0.25, 0.25, 0.375]
        );
        for gate in [
            [8, 0, 1, 1],
            [0, 7, 2, 2],
            [0, 0, 0, 3],
            [2, 0, u32::MAX, 1],
        ] {
            assert!(Fields::from_slice(&make(gate)).is_err());
        }
        assert!(Fields::from_slice(&make([2, 3, 4, 2])[..39]).is_err());
        assert!(Fields::from_slice(&make([2, 3, 4, 2])[..WORLD_HEADER + 1]).is_err());
    }
}
