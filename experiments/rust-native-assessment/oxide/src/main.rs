//! Probe a fixture without creating a decoder or opening a GPU device.
use oxideav_core::{Error, NullCodecResolver};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: probe <media-file>")?;
    let mut demux = oxideav_mp4::demux::open(Box::new(File::open(path)?), &NullCodecResolver)?;
    let streams: Vec<_> = demux.streams().iter().map(|s| {
        let p = &s.params;
        json!({
            "index": s.index, "codec": p.codec_id.as_str(), "range": format!("{:?}", p.color_signal.range),
            "primaries": p.color_signal.primaries.code_point(), "transfer": p.color_signal.transfer.code_point(),
            "matrix": p.color_signal.matrix.code_point(), "size": [p.width, p.height],
            "audio": [p.sample_rate, p.channels.map(u32::from)],
            "time_base": [s.time_base.num(), s.time_base.den()], "duration": s.duration,
            "start": s.start_time, "pixel_format": p.pixel_format.map(|v| format!("{v:?}"))
        })
    }).collect();
    let mut packets = Vec::new();
    loop {
        let packet = match demux.next_packet() {
            Ok(p) => p,
            Err(Error::Eof) => break,
            Err(e) => return Err(e.into()),
        };
        packets.push(json!({
            "stream_index": packet.stream_index, "pts": packet.pts, "dts": packet.dts,
            "duration": packet.duration, "size": packet.data.len(),
            "data_hash": format!("SHA256:{:x}", Sha256::digest(&packet.data))
        }));
    }
    serde_json::to_writer(
        std::io::stdout().lock(),
        &json!({"streams": streams, "packets": packets}),
    )?;
    Ok(())
}
