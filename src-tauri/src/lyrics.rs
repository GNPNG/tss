use crate::SelectedPaths;
use process_wrap::std::{ChildWrapper, CommandWrap};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

const LIMIT: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
pub struct Segment {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Default)]
struct Job {
    cancelled: bool,
    child: Option<Box<dyn ChildWrapper>>,
}

impl Job {
    fn cancel(&mut self) -> Result<(), String> {
        self.cancelled = true;
        if let Some(child) = &mut self.child {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                child.kill().map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Transcriber(Mutex<Option<Arc<Mutex<Job>>>>);

impl Transcriber {
    fn start(&self) -> Result<Arc<Mutex<Job>>, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Err("Transcription is already running.".into());
        }
        let job = Arc::new(Mutex::new(Job::default()));
        *slot = Some(job.clone());
        Ok(job)
    }

    pub fn cancel(&self) -> Result<(), String> {
        if let Some(job) = self.0.lock().map_err(|e| e.to_string())?.as_ref() {
            job.lock().map_err(|e| e.to_string())?.cancel()?;
        }
        Ok(())
    }
}

fn tool_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<_> = env::var_os("PATH")
        .map(|v| env::split_paths(&v).filter(|p| p.is_absolute()).collect())
        .unwrap_or_default();
    #[cfg(windows)]
    let home = env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = env::var_os("HOME");
    if let Some(home) = home {
        dirs.push(PathBuf::from(home).join(".local/bin"));
    }
    #[cfg(target_os = "macos")]
    dirs.extend(["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    #[cfg(windows)]
    if let Some(programs) = env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(programs).join("PowerShell/7"));
    }
    dirs
}

fn tool(dirs: &[PathBuf], name: &str) -> Result<PathBuf, String> {
    dirs.iter()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
        .ok_or_else(|| format!("Cannot find {name}. See local transcription setup in the editor."))
}

fn transcribe_command(audio: &Path, out: &Path) -> Result<Command, String> {
    let dirs = tool_dirs();
    #[cfg(windows)]
    let mut command = {
        let mut cmd = Command::new(tool(&dirs, "pwsh.exe")?);
        cmd.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
            .arg(tool(&dirs, "local-transcribe.ps1")?)
            .arg("-InputFile")
            .arg(audio)
            .arg("-OutputDir")
            .arg(out)
            .args(["-Formats", "json", "-Cpu"]);
        cmd
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut cmd = Command::new("/bin/bash");
        cmd.arg(tool(&dirs, "local-transcribe")?)
            .arg(audio)
            .arg(out)
            .args(["--formats", "json", "--cpu"]);
        cmd
    };
    command
        .env("NO_COLOR", "1")
        .env("PATH", env::join_paths(dirs).map_err(|e| e.to_string())?);
    Ok(command)
}

fn run_child(command: Command, job: &Mutex<Job>) -> Result<ExitStatus, String> {
    let mut command = CommandWrap::from(command);
    #[cfg(unix)]
    command.wrap(process_wrap::std::ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(process_wrap::std::JobObject);
    {
        let mut state = job.lock().map_err(|e| e.to_string())?;
        if state.cancelled {
            return Err("Transcription cancelled.".into());
        }
        state.child = Some(
            command
                .spawn()
                .map_err(|e| format!("Cannot start local-transcribe: {e}"))?,
        );
    }
    loop {
        {
            let mut state = job.lock().map_err(|e| e.to_string())?;
            if state.cancelled {
                return Err("Transcription cancelled.".into());
            }
            let child = state
                .child
                .as_mut()
                .ok_or("Transcription process is missing.")?;
            match child.try_wait() {
                Ok(Some(status)) => {
                    state.child.take();
                    return Ok(status);
                }
                Ok(None) => (),
                Err(e) => {
                    let _ = child.kill();
                    return Err(e.to_string());
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn read_segments(path: &Path) -> Result<Vec<Segment>, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open timed transcript: {e}"))?;
    let mut data = Vec::new();
    file.take(LIMIT as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > LIMIT {
        return Err("Timed transcript exceeds 16 MiB.".into());
    }
    let segments: Vec<Segment> =
        serde_json::from_slice(data.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&data))
            .map_err(|e| format!("Invalid local-transcribe segments: {e}"))?;
    if segments.is_empty()
        || segments.iter().any(|s| {
            !s.start.is_finite()
                || !s.end.is_finite()
                || s.start < 0.0
                || s.end <= s.start
                || s.text.trim().is_empty()
        })
    {
        return Err("The transcript contains empty or invalid timed segments.".into());
    }
    Ok(segments)
}

fn transcribe(audio: &Path, job: &Mutex<Job>) -> Result<Vec<Segment>, String> {
    let dir = tempfile::Builder::new()
        .prefix("tss-lyrics-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    // Keep diagnostics in a separate file from the user-controlled output stem.
    let mut diagnostics = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut cmd = transcribe_command(audio, dir.path())?;
    cmd.stdin(Stdio::null())
        .stdout(diagnostics.try_clone().map_err(|e| e.to_string())?)
        .stderr(diagnostics.try_clone().map_err(|e| e.to_string())?);
    let status = run_child(cmd, job)?;
    if !status.success() {
        let message = match status.code() {
            Some(2) => "The transcript still repeats after retry.",
            Some(3) => "No speech recognized in this recording.",
            _ => "Local transcription failed.",
        };
        let size = diagnostics.metadata().map_err(|e| e.to_string())?.len();
        diagnostics
            .seek(SeekFrom::Start(size.saturating_sub(3072)))
            .map_err(|e| e.to_string())?;
        let mut tail = Vec::new();
        diagnostics
            .read_to_end(&mut tail)
            .map_err(|e| e.to_string())?;
        return Err(format!(
            "{message} {}",
            String::from_utf8_lossy(&tail).trim()
        ));
    }
    let name = audio.file_name().ok_or("Audio filename is missing.")?;
    read_segments(&dir.path().join(name).with_extension("json"))
}

#[tauri::command]
pub async fn transcribe_song(
    audio_path: String,
    paths: State<'_, SelectedPaths>,
    jobs: State<'_, Transcriber>,
) -> Result<Vec<Segment>, String> {
    let audio = PathBuf::from(audio_path);
    let id = {
        let selected = paths.audio.lock().map_err(|e| e.to_string())?;
        if selected.confirmed.as_ref() != Some(&audio) {
            return Err("Choose this audio file first.".into());
        }
        selected.confirmed_id.ok_or("Choose an audio file first.")?
    };
    let job = jobs.start()?;
    let result = tauri::async_runtime::spawn_blocking(move || transcribe(&audio, &job)).await;
    jobs.0.lock().map_err(|e| e.to_string())?.take();
    if paths.audio.lock().map_err(|e| e.to_string())?.confirmed_id != Some(id) {
        return Err("The selected song changed during transcription.".into());
    }
    result.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn cancel_transcription(jobs: State<'_, Transcriber>) -> Result<(), String> {
    jobs.cancel()
}

#[tauri::command]
pub async fn save_reviewed_lyrics(
    app: tauri::AppHandle,
    text: String,
    paths: State<'_, SelectedPaths>,
    audio_path: String,
) -> Result<bool, String> {
    if text.len() > LIMIT || !text.starts_with("WEBVTT\n") {
        return Err("Expected a WebVTT transcript under 16 MiB.".into());
    }
    let path = PathBuf::from(audio_path);
    if paths
        .audio
        .lock()
        .map_err(|e| e.to_string())?
        .confirmed
        .as_ref()
        != Some(&path)
    {
        return Err("Select this recording first.".into());
    }
    let suggested = path.with_extension("vtt");
    let mut picker = app.dialog().file().add_filter("WebVTT lyrics", &["vtt"]);
    if let Some(parent) = suggested.parent() {
        picker = picker.set_directory(parent);
    }
    let selection = picker
        .set_file_name(suggested.file_name().unwrap_or_default().to_string_lossy())
        .blocking_save_file();
    let Some(file) = selection else {
        return Ok(false);
    };
    let mut dst = file.into_path().map_err(|e| e.to_string())?;
    if dst.extension().is_none() {
        dst.set_extension("vtt");
    }
    if !dst
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("vtt"))
    {
        return Err("Choose a .vtt filename.".into());
    }
    let mut staged =
        tempfile::NamedTempFile::new_in(dst.parent().ok_or("Missing output directory")?)
            .map_err(|e| e.to_string())?;
    staged
        .write_all(text.as_bytes())
        .and_then(|_| staged.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    staged.persist(dst).map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_multiple_and_invalid_segments() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("take.json");
        std::fs::write(
            &p,
            br#"[{"start":0.01,"end":1.1,"text":"red"},{"start":2,"end":3,"text":"blue"}]"#,
        )
        .unwrap();
        assert_eq!(read_segments(&p).unwrap().len(), 2);
        std::fs::write(&p, br#"[{"start":4,"end":3,"text":"red"}]"#).unwrap();
        assert!(read_segments(&p).is_err());
    }
    #[test]
    fn cancellation_reaps_a_running_wrapper_and_rejects_a_second_run() {
        let controller = Transcriber::default();
        let job = controller.start().unwrap();
        assert!(controller.start().is_err());
        let running = job.clone();
        let worker = std::thread::spawn(move || {
            #[cfg(unix)]
            let mut cmd = Command::new("/bin/sh");
            #[cfg(unix)]
            cmd.args(["-c", "sleep 30 & wait"]);
            #[cfg(windows)]
            let mut cmd = Command::new("cmd.exe");
            #[cfg(windows)]
            cmd.args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"]);
            run_child(cmd, &running)
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while job.lock().unwrap().child.is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "wrapper did not start"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        controller.cancel().unwrap();
        assert_eq!(
            worker.join().unwrap().unwrap_err(),
            "Transcription cancelled."
        );
        assert!(job
            .lock()
            .unwrap()
            .child
            .as_mut()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_some());
    }
    #[test]
    #[ignore = "uses the installed local-transcribe and speech model"]
    fn installed_transcriber() {
        let p = PathBuf::from(env::var_os("TSS_TRANSCRIBE_FIXTURE").expect("speech fixture path"));
        let segments = transcribe(&p, &Mutex::new(Job::default())).unwrap();
        let text = segments
            .iter()
            .map(|s| s.text.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("glass") && text.contains("wheel"), "{text}");
    }
}
