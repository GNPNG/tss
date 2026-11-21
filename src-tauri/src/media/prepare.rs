use super::{ioerr, MediaError, MediaResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Serialize, Deserialize)]
pub(super) struct Sample {
    pub time_us: i64,
    pub duration_us: i64,
    pub pos: u64,
    pub size: u32,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Track {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<Sample>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Index {
    pub master: Track,
    pub proxy: Option<Track>,
    pub rec709_transfer: bool,
}

fn program(binary: crate::media_tools::Binary) -> MediaResult<Command> {
    let mut c = crate::media_tools::command(binary)?;
    c.stdin(Stdio::null());
    Ok(c)
}

fn probe_args() -> MediaResult<Command> {
    let mut c = program(crate::media_tools::Binary::Ffprobe)?;
    c.args([
        "-v",
        "error",
        "-protocol_whitelist",
        "file,pipe",
        "-select_streams",
        "V:0",
        "-of",
        "json",
    ]);
    Ok(c)
}

fn probe_source(path: &Path) -> MediaResult<Value> {
    let fields = concat!(
        "format=format_name:stream=width,height,pix_fmt,codec_name,color_transfer,",
        "color_primaries,sample_aspect_ratio:stream_tags=alpha_mode"
    );
    let o = probe_args()?
        .args(["-show_entries", fields])
        .arg(path)
        .output()
        .map_err(ioerr)?;
    if !o.status.success() {
        return Err(format!(
            "Could not probe source: {}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    let info: Value = serde_json::from_slice(&o.stdout).map_err(ioerr)?;
    let formats = info["format"]["format_name"].as_str().unwrap_or("");
    if !formats.split(',').any(|f| {
        matches!(
            f,
            "mov"
                | "mp4"
                | "matroska"
                | "webm"
                | "avi"
                | "gif"
                | "apng"
                | "png_pipe"
                | "jpeg_pipe"
                | "webp_pipe"
                | "image2"
                | "tiff_pipe"
                | "bmp_pipe"
        )
    }) {
        return Err(format!("Unsupported visual media container: {formats}."));
    }
    let stream = info["streams"]
        .get(0)
        .ok_or("No visual stream in this file.")?;
    let (w, h) = (
        stream["width"].as_u64().unwrap_or(0),
        stream["height"].as_u64().unwrap_or(0),
    );
    if w == 0 || h == 0 || w.saturating_mul(h) > 33_554_432 {
        return Err("Visual media must have a positive raster of up to 32 megapixels.".into());
    }
    Ok(stream.clone())
}

fn rec709_transfer(source: &Value) -> MediaResult<bool> {
    let primaries = source["color_primaries"].as_str().unwrap_or("unknown");
    if !matches!(primaries, "unknown" | "unspecified" | "bt709") {
        return Err(format!(
            "Convert {primaries} media to sRGB or Rec.709 before import."
        ));
    }
    match source["color_transfer"].as_str().unwrap_or("unknown") {
        "iec61966-2-1" => Ok(false),
        "bt709" | "smpte170m" => Ok(true),
        "unknown" | "unspecified" => {
            let p = source["pix_fmt"].as_str().unwrap_or("");
            let still = matches!(
                source["codec_name"].as_str(),
                Some("png" | "apng" | "mjpeg" | "gif" | "webp" | "tiff" | "bmp")
            );
            Ok(!still && (p.starts_with("yuv") || p.starts_with("nv")))
        }
        other => Err(format!(
            "Convert {other} media to SDR sRGB or Rec.709 before import."
        )),
    }
}

fn encode(source: &Path, info: &Value, dest: &Path, proxy: bool) -> MediaResult<()> {
    let mut c = program(crate::media_tools::Binary::Ffmpeg)?;
    c.args([
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-protocol_whitelist",
        "file,pipe",
    ]);
    // libvpx reads the separate alpha bitstream in WebM.
    if info["tags"]["alpha_mode"].as_str() == Some("1") {
        match info["codec_name"].as_str() {
            Some("vp9") => {
                c.args(["-c:v", "libvpx-vp9"]);
            }
            Some("vp8") => {
                c.args(["-c:v", "libvpx"]);
            }
            _ => (),
        }
    }
    c.arg("-i")
        .arg(source)
        .args(["-map", "0:V:0", "-an", "-sn", "-dn", "-map_metadata", "-1"]);
    let mut filters = vec!["setpts=PTS-STARTPTS"];
    let square_pixels = info["sample_aspect_ratio"]
        .as_str()
        .is_some_and(|sar| !matches!(sar, "N/A" | "1:1" | "0:1"));
    if proxy || square_pixels {
        // Filter covered colour so hidden RGB cannot bleed across the edge of a keyed figure.
        filters.extend(["format=gbrap16le", "premultiply=inplace=1"]);
    }
    if square_pixels {
        filters.push("scale=w='max(1,round(iw*sar))':h=ih");
    }
    if proxy {
        filters.push(
            "scale=w='min(960,iw)':h='min(960,ih)':force_original_aspect_ratio=decrease:flags=area",
        );
    }
    if proxy || square_pixels {
        filters.push("unpremultiply=inplace=1");
    }
    filters.extend(["setsar=1", "format=rgba"]);
    let o = c
        .arg("-vf")
        .arg(filters.join(","))
        .args([
            "-fps_mode",
            "passthrough",
            "-enc_time_base",
            "demux",
            "-c:v",
            "png",
            "-pix_fmt",
            "rgba",
            "-compression_level",
            "2",
            "-threads:v",
            "2",
            "-f",
            "mov",
        ])
        .arg(dest)
        .output()
        .map_err(ioerr)?;
    if !o.status.success() {
        return Err(format!(
            "FFmpeg could not prepare this visual: {}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct Packet {
    pts: i64,
    duration: Option<i64>,
    pos: String,
    size: String,
}
#[derive(Deserialize)]
struct Stream {
    width: u32,
    height: u32,
    time_base: String,
}
#[derive(Deserialize)]
struct PacketIndex {
    packets: Vec<Packet>,
    streams: Vec<Stream>,
}

fn index(path: &Path) -> MediaResult<Track> {
    let out = tempfile::tempfile().map_err(ioerr)?;
    let result = probe_args()?
        .args([
            "-show_entries",
            "packet=pts,duration,pos,size:stream=width,height,time_base",
        ])
        .arg(path)
        .stdout(out.try_clone().map_err(ioerr)?)
        .stderr(Stdio::piped())
        .output()
        .map_err(ioerr)?;
    if !result.status.success() {
        return Err(format!(
            "Cannot index prepared frames: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    use std::io::{Seek, SeekFrom};
    let mut out = out;
    out.seek(SeekFrom::Start(0)).map_err(ioerr)?;
    let data: PacketIndex = serde_json::from_reader(BufReader::new(out)).map_err(ioerr)?;
    let stream = data.streams.first().ok_or("No prepared frame stream.")?;
    let (num, den) = stream
        .time_base
        .split_once('/')
        .ok_or("Invalid video time base.")?;
    let (num, den) = (
        num.parse::<i64>().map_err(ioerr)?,
        den.parse::<i64>().map_err(ioerr)?,
    );
    if num <= 0 || den <= 0 {
        return Err("Invalid video time base.".into());
    }
    let micros = |v: i64| -> MediaResult<i64> {
        i64::try_from((v as i128 * num as i128 * 1_000_000 + den as i128 / 2) / den as i128)
            .map_err(ioerr)
    };
    let mut samples = Vec::with_capacity(data.packets.len());
    let start = data
        .packets
        .first()
        .ok_or("The source contains no frames.")?
        .pts;
    for p in data.packets {
        samples.push(Sample {
            time_us: micros(p.pts.checked_sub(start).ok_or("Invalid frame time.")?)?,
            duration_us: micros(p.duration.unwrap_or(0))?,
            pos: p.pos.parse().map_err(ioerr)?,
            size: p.size.parse().map_err(ioerr)?,
        });
    }
    let track = Track {
        width: stream.width,
        height: stream.height,
        samples,
    };
    validate_track(&track, path)?;
    Ok(track)
}

fn validate_track(track: &Track, path: &Path) -> MediaResult<()> {
    let size = fs::metadata(path).map_err(ioerr)?.len();
    if track.width == 0
        || track.height == 0
        || u64::from(track.width) * u64::from(track.height) > 33_554_432
        || track.samples.first().map(|p| p.time_us) != Some(0)
        || track
            .samples
            .windows(2)
            .any(|p| p[0].time_us >= p[1].time_us)
        || track.samples.iter().any(|p| {
            p.size < 33
                || p.size > 144 * 1024 * 1024
                || p.pos.saturating_add(u64::from(p.size)) > size
                || p.duration_us < 0
                || p.time_us.checked_add(p.duration_us).is_none()
        })
    {
        return Err("Invalid prepared frame index.".into());
    }
    Ok(())
}

pub(super) fn load(dir: &Path) -> MediaResult<Index> {
    let f = File::open(dir.join("frames.json")).map_err(ioerr)?;
    let v: Index = serde_json::from_reader(BufReader::new(f)).map_err(ioerr)?;
    validate_track(&v.master, &dir.join("master.mov"))?;
    if let Some(proxy) = &v.proxy {
        validate_track(proxy, &dir.join("preview.mov"))?;
        if proxy.samples.len() != v.master.samples.len()
            || proxy
                .samples
                .iter()
                .zip(&v.master.samples)
                .any(|(a, b)| a.time_us != b.time_us)
        {
            return Err("Preview sample times differ from the master.".into());
        }
    }
    Ok(v)
}

pub(super) fn prepare(path: &Path, root: &Path, hash: &str) -> MediaResult<(PathBuf, Index)> {
    fs::create_dir_all(root).map_err(ioerr)?;
    let dir = root.join(hash);
    if dir.is_dir() {
        return Ok((dir.clone(), load(&dir)?));
    }
    let info = probe_source(path)?;
    let transfer = rec709_transfer(&info)?;
    let stage = tempfile::Builder::new()
        .prefix(".prepare-")
        .tempdir_in(root)
        .map_err(ioerr)?;
    let master_file = stage.path().join("master.mov");
    encode(path, &info, &master_file, false)?;
    let master = index(&master_file)?;
    let proxy = if master.width.max(master.height) > 960 {
        let p = stage.path().join("preview.mov");
        encode(path, &info, &p, true)?;
        Some(index(&p)?)
    } else {
        None
    };
    if crate::project::digest_file(path).map_err(ioerr)? != hash {
        return Err("Media changed while preparing it. Select it again.".into());
    }
    let v = Index {
        master,
        proxy,
        rec709_transfer: transfer,
    };
    let mut out = File::create(stage.path().join("frames.json")).map_err(ioerr)?;
    serde_json::to_writer(&mut out, &v).map_err(ioerr)?;
    out.flush().map_err(ioerr)?;
    drop(out);
    let checked = load(stage.path())?;
    match fs::rename(stage.path(), &dir) {
        Ok(()) => Ok((dir, checked)),
        Err(_) if dir.is_dir() => Ok((dir.clone(), load(&dir)?)),
        Err(e) => Err(MediaError::from(e.to_string())),
    }
}
