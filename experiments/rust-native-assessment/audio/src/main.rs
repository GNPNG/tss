use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::path::Path;
use std::time::Instant;

use symphonia::core::formats::{probe::Hint, TrackType};
use symphonia::core::io::MediaSourceStream;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err(
            "usage: tss-symphonia-comparison INPUT REFERENCE.f32le [reference_skip_frames]".into(),
        );
    }
    let audio_path = Path::new(&args[0]);
    let start = Instant::now();
    let stream = MediaSourceStream::new(Box::new(File::open(audio_path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = audio_path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }
    let mut demux = symphonia::default::get_probe().probe(
        &hint,
        stream,
        Default::default(),
        Default::default(),
    )?;
    let track = demux
        .default_track(TrackType::Audio)
        .ok_or("no audio track")?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("no audio parameters")?;
    let id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(params, &Default::default())?;
    let mut samples = Vec::<f32>::new();
    let mut sample_format = None;
    while let Some(packet) = demux.next_packet()? {
        if packet.track_id != id {
            continue;
        }
        let audio = decoder.decode(&packet)?;
        let format = (audio.spec().rate(), audio.spec().channels().count());
        if sample_format
            .replace(format)
            .is_some_and(|previous| previous != format)
        {
            return Err("format changed within track".into());
        }
        let previous = samples.len();
        samples.resize(previous + audio.samples_interleaved(), 0.);
        audio.copy_to_slice_interleaved(&mut samples[previous..]);
    }
    let elapsed = start.elapsed();
    let (rate, channels) = sample_format.ok_or("no samples")?;
    let reference = fs::read(&args[1])?;
    if channels == 0 || reference.is_empty() || reference.len() % (4 * channels) != 0 {
        return Err("invalid interleaved float reference".into());
    }
    let reference: Vec<_> = reference
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    println!("input={}", audio_path.display());
    println!("sample_rate={rate} channels={channels} symphonia_frames={} ffmpeg_frames={} elapsed_ms={:.3}",
        samples.len()/channels, reference.len()/channels, elapsed.as_secs_f64()*1e3);
    compare(&reference, &samples, channels, 0)?;
    if let Some(skip) = args.get(2) {
        let frames: usize = skip.to_str().ok_or("invalid frame offset")?.parse()?;
        if frames > 0 {
            compare(&reference, &samples, channels, frames)?;
        }
    }
    Ok(())
}

fn compare(
    reference: &[f32],
    samples: &[f32],
    channels: usize,
    skip: usize,
) -> Result<(), Box<dyn Error>> {
    let samples = samples
        .get(skip.checked_mul(channels).ok_or("offset overflow")?..)
        .ok_or("offset past end")?;
    let common = reference.len().min(samples.len());
    let mut signal = 0.;
    let mut squared_error = 0.;
    let mut peak_error = 0_f64;
    for (&expected, &actual) in reference[..common].iter().zip(&samples[..common]) {
        if !actual.is_finite() || !expected.is_finite() {
            return Err("non-finite value in PCM".into());
        }
        let delta = f64::from(expected) - f64::from(actual);
        signal += f64::from(expected).powi(2);
        squared_error += delta.powi(2);
        peak_error = peak_error.max(delta.abs());
    }
    if common == 0 || signal <= 0. {
        return Err("empty or silent comparison".into());
    }
    println!(
        "skip_symphonia_frames={skip} compared_frames={} snr_db={:.3} peak_abs_error={peak_error:.8} rms_error={:.8}",
        common / channels,
        10. * (signal / squared_error).log10(),
        (squared_error / common as f64).sqrt()
    );
    Ok(())
}
