use base64::{engine::general_purpose::STANDARD, Engine};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
#[cfg(test)]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PNG_BYTES: usize = 64 * 1024 * 1024;
const STDERR_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq)]
enum Input {
    Png,
    Rgba,
}

#[derive(Clone, Default)]
pub struct ExportManager {
    state: Arc<Mutex<Option<ExportJob>>>,
    cancelled: Arc<AtomicBool>,
}

struct ExportJob {
    child: Option<Child>,
    stderr: Option<JoinHandle<String>>,
    temporary: PathBuf,
    output: PathBuf,
    width: u32,
    height: u32,
    input: Input,
    next_frame: u64,
    keep_temporary: bool,
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            drop(child.stdin.take());
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(stderr) = self.stderr.take() {
            let _ = stderr.join();
        }
        if !self.keep_temporary {
            let _ = fs::remove_file(&self.temporary);
        }
    }
}

impl ExportManager {
    pub fn start(
        &self,
        audio: PathBuf,
        output: PathBuf,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<(), String> {
        self.start_with(audio, output, width, height, fps, Input::Png)
    }

    pub fn start_rgba(
        &self,
        audio: PathBuf,
        output: PathBuf,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<(), String> {
        self.start_with(audio, output, width, height, fps, Input::Rgba)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn start_with(
        &self,
        audio: PathBuf,
        output: PathBuf,
        width: u32,
        height: u32,
        fps: u32,
        input: Input,
    ) -> Result<(), String> {
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 || fps == 0 {
            return Err(
                "Width and height must be positive even numbers and FPS must be positive.".into(),
            );
        }
        if !audio.is_file() {
            return Err("The selected audio file is unavailable.".into());
        }
        if output.exists() {
            return Err("The output file already exists. Choose another filename.".into());
        }
        let parent = output
            .parent()
            .filter(|path| path.is_dir())
            .ok_or("The output directory is unavailable.")?;
        let mut state = self.state.lock().map_err(|err| err.to_string())?;
        if state.is_some() {
            return Err("An export is already in progress.".into());
        }
        self.cancelled.store(false, Ordering::SeqCst);

        let mut command = crate::media_tools::command(crate::media_tools::Binary::Ffmpeg)?;
        let temporary = create_temporary_file(parent)?;
        command.args(["-hide_banner", "-loglevel", "error", "-y"]);
        match input {
            Input::Png => {
                command.args(["-f", "image2pipe", "-vcodec", "png"]);
            }
            Input::Rgba => {
                command
                    .args(["-f", "rawvideo", "-pixel_format", "rgba", "-video_size"])
                    .arg(format!("{width}x{height}"));
            }
        }
        command
            .arg("-framerate")
            .arg(fps.to_string())
            .args(["-i", "pipe:0", "-i"])
            .arg(&audio);
        if input == Input::Rgba {
            // The native optics delivers full-range sRGB. Convert its transfer and matrix for video.
            command.args(["-filter_threads", "2", "-threads:v", "2", "-x264-params", "rc-lookahead=8:sync-lookahead=0:threads=2",
                "-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv444p10le,colorspace=iall=bt709:itrc=srgb:irange=tv:all=bt709:range=tv:format=yuv420p",
                "-color_range", "tv", "-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709"]);
        }
        let child = command
            .args([
                "-map",
                "0:v:0",
                "-map",
                "1:a:0",
                "-c:v",
                "libx264",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "256k",
                "-movflags",
                "+faststart",
                "-shortest",
                "-f",
                "mp4",
            ])
            .arg(&temporary)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(err) => {
                let _ = fs::remove_file(&temporary);
                return Err(format!(
                    "Could not start FFmpeg: {err}. Open FFmpeg media tools in the editor and recheck."
                ));
            }
        };
        let stderr = child.stderr.take().map(|mut pipe| {
            thread::spawn(move || {
                let mut result = Vec::new();
                let mut buffer = [0; 4096];
                while let Ok(count) = pipe.read(&mut buffer) {
                    if count == 0 {
                        break;
                    }
                    let remaining = STDERR_LIMIT.saturating_sub(result.len());
                    result.extend_from_slice(&buffer[..count.min(remaining)]);
                }
                String::from_utf8_lossy(&result).trim().to_string()
            })
        });
        *state = Some(ExportJob {
            child: Some(child),
            stderr,
            temporary,
            output,
            width,
            height,
            input,
            next_frame: 0,
            keep_temporary: false,
        });
        Ok(())
    }

    pub fn append(&self, index: u64, png_base64: &str) -> Result<(), String> {
        let data = png_base64
            .strip_prefix("data:image/png;base64,")
            .unwrap_or(png_base64);
        if data.len() > ((MAX_PNG_BYTES + 2) / 3) * 4 {
            return Err("The PNG frame is too large.".into());
        }
        let png = STANDARD
            .decode(data)
            .map_err(|_| "The frame is not valid base64.")?;
        self.append_png_bytes(index, &png)
    }

    /// Send a native renderer's PNG bytes through the same ordered export path.
    pub fn append_png(&self, index: u64, png: &[u8]) -> Result<(), String> {
        if png.len() > MAX_PNG_BYTES {
            return Err("The PNG frame is too large.".into());
        }
        self.append_png_bytes(index, png)
    }

    fn append_png_bytes(&self, index: u64, png: &[u8]) -> Result<(), String> {
        self.append_bytes(index, png, Input::Png)
    }

    pub fn append_rgba(&self, index: u64, rgba: &[u8]) -> Result<(), String> {
        self.append_bytes(index, rgba, Input::Rgba)
    }

    fn append_bytes(&self, index: u64, bytes: &[u8], input: Input) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|err| err.to_string())?;
        let job = state.as_mut().ok_or("No export is in progress.")?;
        if index != job.next_frame {
            return Err(format!(
                "Expected frame {}, received {index}.",
                job.next_frame
            ));
        }
        if input != job.input {
            return Err("Unexpected frame format for this export.".into());
        }
        match input {
            Input::Png => validate_png(bytes, job.width, job.height)?,
            Input::Rgba
                if bytes.len() as u64 != u64::from(job.width) * u64::from(job.height) * 4 =>
            {
                return Err("RGBA frame dimensions do not match the export.".into());
            }
            Input::Rgba => (),
        }
        let result = job
            .child
            .as_mut()
            .and_then(|child| child.stdin.as_mut())
            .ok_or("FFmpeg input is closed.")?
            .write_all(bytes);
        if let Err(err) = result {
            let mut failed = state.take().unwrap();
            let detail = failed.stop_and_collect();
            return Err(format!(
                "FFmpeg could not accept frame {index}: {err}{}",
                format_detail(&detail)
            ));
        }
        job.next_frame += 1;
        Ok(())
    }

    pub fn finish(&self) -> Result<(), String> {
        self.finish_with_cancel(&self.cancelled)
    }

    pub fn finish_with_cancel(&self, cancel: &AtomicBool) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|err| err.to_string())?;
        let mut job = state.take().ok_or("No export is in progress.")?;
        if job.next_frame == 0 {
            return Err("No frames were supplied for export.".into());
        }
        let mut child = job.child.take().ok_or("FFmpeg process is unavailable.")?;
        drop(child.stdin.take());
        let status = loop {
            if cancel.load(Ordering::SeqCst) || self.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Export cancelled.".into());
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(std::time::Duration::from_millis(50)),
                Err(err) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("Could not finish FFmpeg: {err}"));
                }
            }
        };
        let detail = job
            .stderr
            .take()
            .and_then(|thread| thread.join().ok())
            .unwrap_or_default();
        if !status.success() {
            return Err(format!(
                "FFmpeg failed ({status}){}",
                format_detail(&detail)
            ));
        }
        if cancel.load(Ordering::SeqCst) || self.is_cancelled() {
            return Err("Export cancelled.".into());
        }
        if let Err(error) = publish_output(&job.temporary, &job.output, cancel) {
            if !cancel.load(Ordering::SeqCst) && !self.is_cancelled() {
                job.keep_temporary = true;
                return Err(format!(
                    "{error} Completed render retained at {}",
                    job.temporary.display()
                ));
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), String> {
        self.cancelled.store(true, Ordering::SeqCst);
        let mut state = self.state.lock().map_err(|err| err.to_string())?;
        drop(state.take());
        Ok(())
    }
}

impl ExportJob {
    fn stop_and_collect(&mut self) -> String {
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take());
            let _ = child.wait();
        }
        self.stderr
            .take()
            .and_then(|thread| thread.join().ok())
            .unwrap_or_default()
    }
}

fn publish_output(temporary: &Path, output: &Path, cancelled: &AtomicBool) -> Result<(), String> {
    match fs::hard_link(temporary, output) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            Err("The output file already exists. Choose another filename.".into())
        }
        Err(_) => {
            let mut input = fs::File::open(temporary)
                .map_err(|err| format!("Could not read completed render: {err}."))?;
            copy_to_new(&mut input, output, cancelled)
        }
    }
}

fn copy_to_new(input: &mut impl Read, output: &Path, cancelled: &AtomicBool) -> Result<(), String> {
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|err| format!("Could not create output without overwriting: {err}."))?;
    let mut buffer = vec![0; 1024 * 1024];
    let result = (|| -> Result<(), String> {
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err("Export cancelled.".into());
            }
            let count = input
                .read(&mut buffer)
                .map_err(|err| format!("Could not read completed render: {err}."))?;
            if count == 0 {
                break;
            }
            destination
                .write_all(&buffer[..count])
                .map_err(|err| format!("Could not write output: {err}."))?;
        }
        destination
            .sync_all()
            .map_err(|err| format!("Could not finish output: {err}."))?;
        Ok(())
    })();
    drop(destination);
    if let Err(error) = result {
        if let Err(cleanup_error) = fs::remove_file(output) {
            return Err(format!(
                "{error} Could not remove incomplete output {}: {cleanup_error}.",
                output.display()
            ));
        }
        return Err(error);
    }
    Ok(())
}

fn create_temporary_file(parent: &Path) -> Result<PathBuf, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    for attempt in 0..100 {
        let path = parent.join(format!(".tss-{}-{now}-{attempt}.mp4", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("Cannot create a temporary output: {err}")),
        }
    }
    Err("Cannot allocate a temporary output filename.".into())
}

fn validate_png(png: &[u8], width: u32, height: u32) -> Result<(), String> {
    if png.len() < 24 || !png.starts_with(b"\x89PNG\r\n\x1a\n") || &png[12..16] != b"IHDR" {
        return Err("The frame is not a PNG image.".into());
    }
    let actual_width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let actual_height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    if actual_width != width || actual_height != height {
        return Err(format!(
            "Expected a {width}x{height} frame, received {actual_width}x{actual_height}."
        ));
    }
    Ok(())
}

fn format_detail(detail: &str) -> String {
    if detail.is_empty() {
        String::new()
    } else {
        format!(": {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "simulated read failure",
            ))
        }
    }

    #[test]
    fn png_validation_rejects_bad_data_and_wrong_dimensions() {
        let mut image = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        image.extend_from_slice(&64_u32.to_be_bytes());
        image.extend_from_slice(&32_u32.to_be_bytes());
        assert!(validate_png(&image, 64, 32).is_ok());
        assert!(validate_png(&image, 32, 64).is_err());
        assert!(validate_png(&image[..10], 64, 32).is_err());
    }

    #[test]
    fn raw_wheel_frames_mux_with_audio_and_rec709_signaling() {
        if Command::new("ffprobe")
            .arg("-version")
            .stdout(Stdio::null())
            .status()
            .is_err()
        {
            return;
        }
        let dir = std::env::temp_dir().join(format!(
            "tss-raw-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let audio = dir.join("tone.wav");
        let out = dir.join("wheel.mp4");
        assert!(Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.2"
            ])
            .arg(&audio)
            .status()
            .unwrap()
            .success());
        let manager = ExportManager::default();
        manager.start_rgba(audio, out.clone(), 64, 32, 30).unwrap();
        assert!(manager.append_png(0, &[]).is_err());
        assert!(manager.append_rgba(0, &[0; 3]).is_err());
        let mut pixels = vec![0u8; 64 * 32 * 4];
        for frame in 0..6 {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[215, frame as u8 * 35, 80, 255]);
            }
            manager.append_rgba(frame, &pixels).unwrap();
        }
        manager.finish().unwrap();
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=codec_name,color_space,color_transfer,color_primaries,nb_frames",
                "-of",
                "default",
            ])
            .arg(out)
            .output()
            .unwrap();
        let info = String::from_utf8(probe.stdout).unwrap();
        assert!(
            probe.status.success()
                && [
                    "codec_name=h264",
                    "codec_name=aac",
                    "color_space=bt709",
                    "color_transfer=bt709",
                    "color_primaries=bt709",
                    "nb_frames=6"
                ]
                .iter()
                .all(|v| info.contains(v)),
            "{info}"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_and_base64_frames_share_order_and_png_validation() {
        let manager = ExportManager::default();
        *manager.state.lock().unwrap() = Some(ExportJob {
            child: None,
            stderr: None,
            temporary: PathBuf::new(),
            output: PathBuf::new(),
            width: 64,
            height: 32,
            input: Input::Png,
            next_frame: 0,
            keep_temporary: true,
        });
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&64_u32.to_be_bytes());
        png.extend_from_slice(&32_u32.to_be_bytes());
        let encoded = STANDARD.encode(&png);

        assert_eq!(manager.append(1, &encoded), manager.append_png(1, &png));
        assert!(manager
            .append_png(1, &png)
            .unwrap_err()
            .contains("Expected frame 0"));
        let mut wrong_size = png.clone();
        wrong_size[19] = 32;
        assert_eq!(
            manager.append(0, &STANDARD.encode(&wrong_size)),
            manager.append_png(0, &wrong_size)
        );
        assert!(manager
            .append_png(0, &wrong_size)
            .unwrap_err()
            .contains("Expected a 64x32"));
        assert_eq!(manager.append(0, &encoded), manager.append_png(0, &png));
        assert!(manager
            .append_png(0, &png)
            .unwrap_err()
            .contains("FFmpeg input is closed"));
        assert!(manager
            .append(0, "!")
            .unwrap_err()
            .contains("not valid base64"));
    }

    #[test]
    fn export_preserves_existing_output_and_cleans_up_cancelled_job() {
        if Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            return;
        }
        let parent = std::env::temp_dir().join(format!(
            "tss-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&parent).unwrap();
        let audio = parent.join("audio.wav");
        fs::write(&audio, b"test").unwrap();
        let output = parent.join("output.mp4");
        fs::write(&output, b"existing").unwrap();
        let manager = ExportManager::default();
        assert!(manager
            .start(audio.clone(), output.clone(), 64, 64, 30)
            .is_err());
        assert_eq!(fs::read(&output).unwrap(), b"existing");
        let cancelled = parent.join("cancelled.mp4");
        manager.start(audio, cancelled.clone(), 64, 64, 30).unwrap();
        manager.cancel().unwrap();
        assert!(!cancelled.exists());
        assert!(!fs::read_dir(&parent).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tss-")));
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn fallback_copy_never_overwrites_and_removes_failed_or_cancelled_output() {
        let parent = std::env::temp_dir().join(format!(
            "tss-copy-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&parent).unwrap();
        let output = parent.join("output.mp4");
        let cancelled = AtomicBool::new(false);

        fs::write(&output, b"existing").unwrap();
        assert!(copy_to_new(&mut &b"new"[..], &output, &cancelled).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"existing");
        fs::remove_file(&output).unwrap();

        assert!(copy_to_new(&mut FailingReader, &output, &cancelled).is_err());
        assert!(!output.exists());
        cancelled.store(true, Ordering::SeqCst);
        assert!(copy_to_new(&mut &b"new"[..], &output, &cancelled).is_err());
        assert!(!output.exists());
        cancelled.store(false, Ordering::SeqCst);
        copy_to_new(&mut &b"complete"[..], &output, &cancelled).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"complete");
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn cancellation_interrupts_finalization_and_cleans_temporary_output() {
        if Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            return;
        }
        let parent = std::env::temp_dir().join(format!(
            "tss-cancel-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&parent).unwrap();
        let temporary = create_temporary_file(&parent).unwrap();
        let output = parent.join("output.mp4");
        let child = Command::new("ffmpeg")
            .args([
                "-loglevel",
                "error",
                "-re",
                "-f",
                "lavfi",
                "-i",
                "sine=duration=5",
                "-f",
                "null",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let manager = ExportManager::default();
        *manager.state.lock().unwrap() = Some(ExportJob {
            child: Some(child),
            stderr: None,
            temporary: temporary.clone(),
            output: output.clone(),
            width: 64,
            height: 64,
            input: Input::Png,
            next_frame: 1,
            keep_temporary: false,
        });
        let running = manager.clone();
        let finish = thread::spawn(move || running.finish());
        let mut finishing = false;
        for _ in 0..100 {
            if manager.state.try_lock().is_err() {
                finishing = true;
                break;
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(finishing, "finalization did not start");
        manager.cancel().unwrap();
        assert!(finish.join().unwrap().unwrap_err().contains("cancelled"));
        assert!(!temporary.exists());
        assert!(!output.exists());
        fs::remove_dir_all(parent).unwrap();
    }
}
