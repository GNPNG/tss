//! Direct conversion from independently solved coverage to the wheel shader's
//! top-down RG8 signed-distance atlas. Red stores rear oil, green stores front oil.
use crate::Field;
use std::io;

unsafe extern "C" {
    fn tss_live_optical_channel(
        coverage: *const u8,
        source_side: i32,
        side: i32,
        gate_height: i32,
        channel: u32,
        rg: *mut u8,
    ) -> i32;
}

#[derive(Copy, Clone, Debug)]
pub struct Atlas {
    side: usize,
}

impl Atlas {
    /// Multiples of 44 preserve the original centered 512 × 288/704² projection.
    pub fn new(side: usize) -> io::Result<Self> {
        if !(44..=704).contains(&side) || side % 44 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "optical side must be a multiple of 44 in 44..=704",
            ));
        }
        Ok(Self { side })
    }

    pub fn side(self) -> usize {
        self.side
    }

    /// Left, top, width, height in atlas texels. Positive x is right and positive y down.
    pub fn view(self) -> [u32; 4] {
        let unit = self.side as u32 / 44;
        [6 * unit, 13 * unit, 32 * unit, 18 * unit]
    }

    pub fn byte_len(self) -> usize {
        2 * self.side * self.side
    }

    pub fn pack(self, rear: &Field, front: &Field, rg: &mut [u8]) -> io::Result<()> {
        if rg.len() != self.byte_len() || rear.tick != front.tick {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "atlas byte count or simultaneous cell ticks disagree",
            ));
        }
        for (channel, field) in [rear, front].into_iter().enumerate() {
            if !(1..=256).contains(&field.width)
                || field.width != field.height
                || field.coverage.len() != field.width * field.height
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid full-slide field",
                ));
            }
            // The C++ call reads exactly width² input bytes, writes the chosen
            // channel within 2*side² output bytes, and retains neither buffer.
            let status = unsafe {
                tss_live_optical_channel(
                    field.coverage.as_ptr(),
                    field.width as i32,
                    self.side as i32,
                    self.view()[3] as i32,
                    channel as u32,
                    rg.as_mut_ptr(),
                )
            };
            if status != 0 {
                return Err(io::Error::other("CImg optical distance transform failed"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_distance_has_the_original_sign_channel_order_and_top_row() {
        let coverage = |f: fn(usize, usize) -> bool| Field {
            width: 32,
            height: 32,
            tick: 4,
            seconds: 4. / 24.,
            phase_fraction: 0.5,
            coverage: (0..32 * 32)
                .map(|i| if f(i % 32, i / 32) { 255 } else { 0 })
                .collect(),
        };
        let rear = coverage(|x, _| x >= 16);
        let front = coverage(|_, y| y >= 16);
        let atlas = Atlas::new(176).unwrap();
        let mut output = vec![0; atlas.byte_len()];
        atlas.pack(&rear, &front, &mut output).unwrap();
        let at = |x, y| {
            let i: usize = 2 * (y * atlas.side() + x);
            [output[i], output[i + 1]]
        };
        assert_eq!(atlas.view(), [24, 52, 128, 72]);
        assert_eq!(at(20, 20), [0, 0]);
        assert_eq!(at(150, 20), [255, 0]);
        assert_eq!(at(20, 150), [0, 255]);
        assert_eq!(at(150, 150), [255, 255]);
        assert!(at(87, 30)[0] < 128 && at(88, 30)[0] >= 128);
        assert!(at(30, 87)[1] < 128 && at(30, 88)[1] >= 128);
        let mut uneven = front;
        uneven.tick += 1;
        assert!(atlas.pack(&rear, &uneven, &mut output).is_err());
    }
}
