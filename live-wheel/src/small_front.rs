use super::{extent, gpu};
use std::io::Read;

pub(super) struct SmallFront {
    distances: Vec<u8>,
    upload: Vec<u8>,
    slots: [Option<u32>; 2],
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
            slots: [None; 2],
        })
    }

    pub fn frames(
        &mut self,
        queue: &gpu::Queue,
        texture: &gpu::Texture,
        a: u32,
        b: u32,
    ) -> [u32; 2] {
        // GPU array layers 0–1 retain the original wheel. Layers 2–3 contain
        // only the front film, using the green channel expected by the optics.
        let pixels = self.upload.len() / 2;
        cache_slots(&mut self.slots, [a, b], |slot, frame| {
            let offset = frame as usize * pixels;
            for (dst, &value) in self
                .upload
                .chunks_exact_mut(2)
                .zip(&self.distances[offset..offset + pixels])
            {
                dst[1] = value;
            }
            upload_frame(queue, texture, [704, 704], slot + 2, &self.upload);
        })
        .map(|slot| slot + 2)
    }
}

pub(super) fn cache_slots(
    slots: &mut [Option<u32>; 2],
    wanted: [u32; 2],
    mut upload: impl FnMut(u32, u32),
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
