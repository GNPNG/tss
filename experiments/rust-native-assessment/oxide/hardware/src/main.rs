//! Encode exactly three 320x180 I420 frames. Exits when hardware is unconfirmed.
use oxideav_core::{
    CodecId, CodecParameters, Encoder, Error, Frame, PixelFormat, Rational, VideoFrame, VideoPlane,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Instant;

extern "C" {
    fn tss_hardware_confirmed() -> bool;
}

const WIDTH: usize = 320;
const HEIGHT: usize = 180;
const Y: usize = WIDTH * HEIGHT;
const UV: usize = Y / 4;

fn drain(
    encoder: &mut dyn Encoder,
    sink: &mut impl Write,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut count = 0;
    loop {
        match encoder.receive_packet() {
            Ok(packet) => {
                sink.write_all(&packet.data)?;
                eprintln!(
                    "packet={} pts={:?} dts={:?} duration={:?}",
                    packet.data.len(),
                    packet.pts,
                    packet.dts,
                    packet.duration
                );
                count += 1;
            }
            Err(Error::NeedMore | Error::Eof) => return Ok(count),
            Err(err) => return Err(err.into()),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = fs::read(args.next().ok_or("missing I420 input")?)?;
    if input.len() != 3 * (Y + 2 * UV) {
        return Err("requires three 320x180 I420 frames".into());
    }
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.next().ok_or("missing new H264 path")?)?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let mut params = CodecParameters::video(CodecId::new("h264"));
    params.width = Some(WIDTH as u32);
    params.height = Some(HEIGHT as u32);
    params.frame_rate = Some(Rational::new(30, 1));
    params.pixel_format = Some(PixelFormat::Yuv420P);
    params.options.insert("hardware", "require");
    let start = Instant::now();
    let mut encoder = oxideav_videotoolbox::encoder::make_h264_encoder(&params)?;
    let mut count = 0;
    for (index, pixels) in input.chunks_exact(Y + 2 * UV).enumerate() {
        let plane = |data: &[u8], stride| VideoPlane {
            data: data.to_vec(),
            stride,
        };
        let frame = VideoFrame {
            pts: Some(index as i64 * 1_000_000 / 30),
            planes: vec![
                plane(&pixels[..Y], WIDTH),
                plane(&pixels[Y..Y + UV], WIDTH / 2),
                plane(&pixels[Y + UV..], WIDTH / 2),
            ],
        };
        encoder.send_frame(&Frame::Video(frame))?;
        count += drain(encoder.as_mut(), &mut out)?;
    }
    encoder.flush()?;
    count += drain(encoder.as_mut(), &mut out)?;
    eprintln!(
        "output_packets={count} total_encode_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    if count != 3 || !unsafe { tss_hardware_confirmed() } {
        return Err("did not confirm all three packets on a hardware encoder".into());
    }
    Ok(())
}
