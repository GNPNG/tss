//! CPU-only codec control. Stdin is exactly 60 successive 160×90 I420 frames.
use rusty_h264_common::YuvFrame;
use rusty_h264_encoder::{Encoder, EncoderConfig, Preset};
use std::io::{self, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (160, 90);
    let mut config = EncoderConfig::new(width, height);
    config.preset = Preset::Fast;
    config.qp = 22;
    let mut encoder = Encoder::new(config).map_err(|e| format!("{e:?}"))?;
    let mut frame = YuvFrame::black(width, height);
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    for _ in 0..60 {
        input.read_exact(&mut frame.y)?;
        input.read_exact(&mut frame.u)?;
        input.read_exact(&mut frame.v)?;
        output.write_all(&encoder.encode(&frame))?;
    }
    if input.read(&mut [0u8; 1])? != 0 {
        return Err("expected exactly 60 frames".into());
    }
    output.write_all(&encoder.flush())?;
    output.flush()?;
    Ok(())
}
