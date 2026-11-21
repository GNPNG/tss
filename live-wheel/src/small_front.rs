use super::{extent, gpu};
use std::io::Read;

pub(super) struct SmallFront {
    distances: Vec<u8>,
    upload: Vec<u8>,
}

impl SmallFront {
    pub fn load() -> Result<Self, String> {
        const PIXELS: usize = 704 * 704;
        const BYTES: usize = 96 * PIXELS;
        let gzip = include_bytes!("../assets/coral-small-front.r8.gz");
        let mut distances = Vec::with_capacity(BYTES);
        flate2::read::GzDecoder::new(&gzip[..])
            .take((BYTES + 1) as u64)
            .read_to_end(&mut distances)
            .map_err(|e| format!("Small coral front film: {e}"))?;
        if distances.len() != BYTES {
            return Err("Incorrect dimensions for the small coral front film".into());
        }
        Ok(Self {
            distances,
            upload: vec![0; 2 * PIXELS],
        })
    }

    pub fn frame(&mut self, frame: u32) -> &[u8] {
        let pixels = self.upload.len() / 2;
        let offset = frame as usize * pixels;
        for (dst, &value) in self
            .upload
            .chunks_exact_mut(2)
            .zip(&self.distances[offset..offset + pixels])
        {
            dst[1] = value;
        }
        &self.upload
    }
}

pub(super) fn cache_slots<T: Copy + Eq>(
    slots: &mut [Option<T>; 2],
    wanted: [T; 2],
    mut upload: impl FnMut(u32, T),
) -> [u32; 2] {
    std::array::from_fn(|i| {
        let frame = wanted[i];
        if let Some(hit) = slots.iter().position(|s| *s == Some(frame)) {
            return hit as u32;
        }
        let slot = slots
            .iter()
            .position(|s| *s != Some(wanted[1 - i]))
            .unwrap_or(0);
        upload(slot as u32, frame);
        slots[slot] = Some(frame);
        slot as u32
    })
}

pub(super) fn upload_frame(
    queue: &gpu::Queue,
    texture: &gpu::Texture,
    size: [u32; 2],
    slot: u32,
    bytes: &[u8],
) {
    queue.write_texture(
        gpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: gpu::Origin3d {
                x: 0,
                y: 0,
                z: slot,
            },
            aspect: gpu::TextureAspect::All,
        },
        bytes,
        gpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * 2),
            rows_per_image: Some(size[1]),
        },
        extent(size, 1),
    );
}
