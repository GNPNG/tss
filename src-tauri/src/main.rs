#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod advisor;
mod legend_catalog;
mod export;
mod export_smoke;
mod lyrics;
mod media;
mod live;
mod performer;
mod media_audio;
mod media_tools;
mod project;

use export::ExportManager;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct SelectedPaths {
    audio: Mutex<AudioSelection>,
    output: Mutex<Option<PathBuf>>,
}

#[derive(Default)]
struct AudioSelection {
    last_request: u64,
    pending: Option<(u64, PathBuf)>,
    confirmed: Option<PathBuf>,
    confirmed_id: Option<u64>,
}

impl AudioSelection {
    fn begin(&mut self) -> u64 {
        self.last_request = self.last_request.wrapping_add(1);
        self.pending = None;
        self.last_request
    }

    fn select(&mut self, id: u64, path: PathBuf) -> Option<(u64, String)> {
        if id != self.last_request {
            return None;
        }
        self.pending = Some((id, path.clone()));
        Some((id, path.to_string_lossy().into_owned()))
    }

    fn pending_path(&self, id: u64) -> Result<PathBuf, String> {
        match &self.pending {
            Some((pending_id, path)) if *pending_id == id && self.last_request == id => {
                Ok(path.clone())
            }
            _ => Err("Audio selection was superseded. Choose the file again.".into()),
        }
    }

    fn confirm(&mut self, id: u64) -> Result<(), String> {
        self.confirmed = Some(self.pending_path(id)?);
        self.confirmed_id = Some(id);
        self.pending = None;
        Ok(())
    }
}

#[tauri::command]
async fn choose_audio(
    app: tauri::AppHandle,
    paths: State<'_, SelectedPaths>,
) -> Result<Option<(u64, String)>, String> {
    let id = paths.audio.lock().map_err(|err| err.to_string())?.begin();
    let selected = app
        .dialog()
        .file()
        .add_filter("Audio", &["mp3", "m4a"])
        .blocking_pick_file();
    let Some(file) = selected else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|err| err.to_string())?;
    let path = std::fs::canonicalize(path).map_err(|err| format!("Cannot open audio: {err}"))?;
    if !path.is_file() {
        return Err("The selected audio is not a file.".into());
    }
    Ok(paths
        .audio
        .lock()
        .map_err(|err| err.to_string())?
        .select(id, path))
}

#[tauri::command]
async fn read_pending_audio(
    paths: State<'_, SelectedPaths>,
    selection_id: u64,
) -> Result<tauri::ipc::Response, String> {
    let path = paths
        .audio
        .lock()
        .map_err(|err| err.to_string())?
        .pending_path(selection_id)?;
    let bytes = std::fs::read(path).map_err(|err| format!("Cannot read audio: {err}"))?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
async fn confirm_audio(paths: State<'_, SelectedPaths>, selection_id: u64) -> Result<(), String> {
    paths
        .audio
        .lock()
        .map_err(|err| err.to_string())?
        .confirm(selection_id)
}

#[tauri::command]
async fn read_timed_lyrics(
    paths: State<'_, SelectedPaths>,
    selection_id: u64,
) -> Result<Option<(String, String)>, String> {
    let audio = {
        let state = paths.audio.lock().map_err(|e| e.to_string())?;
        if state.confirmed_id != Some(selection_id) {
            return Ok(None);
        }
        state.confirmed.clone().ok_or("Choose audio first")?
    };
    tauri::async_runtime::spawn_blocking(move || timed_lyrics_for(&audio))
        .await
        .map_err(|err| err.to_string())?
}

fn timed_lyrics_for(audio: &std::path::Path) -> Result<Option<(String, String)>, String> {
    use std::io::{ErrorKind, Read};
    const LIMIT: u64 = 16 * 1024 * 1024;
    for ext in ["vtt", "srt", "json"] {
        let path = audio.with_extension(ext);
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
            Ok(m) if !m.is_file() => continue,
            Ok(_) => (),
        }
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut data = String::new();
        file.take(LIMIT + 1)
            .read_to_string(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() as u64 > LIMIT {
            return Err("Timed lyric file exceeds the 16 MiB automatic import limit.".into());
        }
        return Ok(Some((
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            data,
        )));
    }
    Ok(None)
}

#[tauri::command]
async fn choose_export_path(
    app: tauri::AppHandle,
    paths: State<'_, SelectedPaths>,
) -> Result<Option<String>, String> {
    let selected = app
        .dialog()
        .file()
        .set_file_name("sacred-swirls.mp4")
        .add_filter("MP4 video", &["mp4"])
        .blocking_save_file();
    let Some(file) = selected else {
        return Ok(None);
    };
    let mut path = file.into_path().map_err(|err| err.to_string())?;
    if path.extension().is_none() {
        path.set_extension("mp4");
    }
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
    {
        return Err("Choose a filename ending in .mp4.".into());
    }
    *paths.output.lock().map_err(|err| err.to_string())? = Some(path.clone());
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn start_export(
    paths: State<'_, SelectedPaths>,
    manager: State<'_, ExportManager>,
    audio_path: String,
    output_path: String,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<(), String> {
    let (audio, output) = selected_export_paths(&paths, &audio_path, &output_path)?;
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.start(audio, output, width, height, fps))
        .await
        .map_err(|err| err.to_string())?
}

fn selected_export_paths(
    paths: &SelectedPaths,
    audio_path: &str,
    output_path: &str,
) -> Result<(PathBuf, PathBuf), String> {
    let audio = paths
        .audio
        .lock()
        .map_err(|err| err.to_string())?
        .confirmed
        .clone()
        .ok_or("Choose an audio file first.")?;
    let output = paths
        .output
        .lock()
        .map_err(|err| err.to_string())?
        .clone()
        .ok_or("Choose an output file first.")?;
    if audio != PathBuf::from(audio_path) || output != PathBuf::from(output_path) {
        return Err("The selected files changed. Choose them again.".into());
    }
    Ok((audio, output))
}

#[tauri::command]
async fn append_export_frame(
    manager: State<'_, ExportManager>,
    index: u64,
    png_base64: String,
) -> Result<(), String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.append(index, &png_base64))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn finish_export(manager: State<'_, ExportManager>) -> Result<(), String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.finish())
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn cancel_export(manager: State<'_, ExportManager>) -> Result<(), String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.cancel())
        .await
        .map_err(|err| err.to_string())?
}

fn main() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = if std::env::args_os().any(|a| a == "--verify-mac-preview") {
        live::watch_verification();
        builder.on_page_load(|view, page| {
            if view.label() == "main" && matches!(page.event(), tauri::webview::PageLoadEvent::Finished) {
                let window = view.window();
                if let Err(error) = window.show().and_then(|_| window.set_focus()) {
                    eprintln!("Could not show the Mac preview verification window: {error}");
                }
            }
        })
    } else {
        builder
    };
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(lyrics::Transcriber::default())
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                let _ = window.state::<lyrics::Transcriber>().cancel();
            }
        })
        .manage(SelectedPaths::default())
        .manage(project::ProjectDirectory::default())
        .manage(ExportManager::default())
        .manage(live::Host::default())
        .manage(performer::Performer::default())
        .manage(live::movie::ExportControl::default())
        .invoke_handler(tauri::generate_handler![
            legend_catalog::available_legend_packs,
            export_smoke::select_wheel_export_fixture,
            export_smoke::finish_wheel_export_verification,
            live::set_live_wheel,
            media_audio::set_visual_audio_features,
            media_tools::check_media_tools,
            media_tools::install_media_tools,
            performer::choose_performer_tls,
            performer::start_performer,
            performer::start_performer_verification,
            performer::performer_verification_step,
            performer::stop_performer,
            live::set_projector_output,
            live::set_stereo_output,
            live::stereo_startup_requested,
            live::verify_projector_visibility,
            live::set_live_wheel_capture_enabled,
            live::replay_live_wheel_gesture,
            live::perform_live_wheel_gesture,
            live::set_live_wheel_pressure,
            live::forget_live_wheel_layer,
            live::live_wheel_stats,
            live::live_simulation_available,
            live::live_wheel_benchmark_config,
            live::plates::export_native_wheel_plates,
            live::plates::preview_wheel_plates,
            live::plates::print_geometry,
            live::movie::export_native_wheel,
            live::movie::cancel_native_wheel_export,
            choose_audio,
            read_pending_audio,
            confirm_audio,
            read_timed_lyrics,
            media::choose_visual_media,
            media::proof_visual_grants,
            media::resolve_visual_media,
            media::relink_visual_media,
            advisor::suggest_visual_layer,
            lyrics::transcribe_song,
            lyrics::cancel_transcription,
            lyrics::save_reviewed_lyrics,
            project::open_project,
            project::save_project,
            project::remember_project_audio,
            project::clear_project_audio,
            project::project_audio_digest,
            choose_export_path,
            start_export,
            append_export_frame,
            finish_export,
            cancel_export
        ])
        .run(tauri::generate_context!())
        .expect("failed to run The Sacred Swirls");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_lyric_sidecars_follow_the_selected_audio_stem() {
        let dir = std::env::temp_dir().join(format!(
            "tss-sidecar-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let audio = dir.join("verse.take.m4a");
        std::fs::write(dir.join("other.vtt"), "other").unwrap();
        assert_eq!(timed_lyrics_for(&audio).unwrap(), None);
        std::fs::write(audio.with_extension("srt"), "captions").unwrap();
        assert_eq!(
            timed_lyrics_for(&audio).unwrap(),
            Some(("verse.take.srt".into(), "captions".into()))
        );
        std::fs::write(audio.with_extension("vtt"), "WEBVTT").unwrap();
        assert_eq!(
            timed_lyrics_for(&audio).unwrap(),
            Some(("verse.take.vtt".into(), "WEBVTT".into()))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn failed_or_superseded_selection_preserves_confirmed_audio() {
        let mut selection = AudioSelection::default();
        let first = selection.begin();
        selection.select(first, PathBuf::from("first.m4a"));
        selection.confirm(first).unwrap();

        let second = selection.begin();
        selection.select(second, PathBuf::from("second.m4a"));
        let third = selection.begin();
        assert!(selection.confirm(second).is_err());
        assert!(selection
            .select(second, PathBuf::from("late.m4a"))
            .is_none());
        assert_eq!(selection.confirmed, Some(PathBuf::from("first.m4a")));
        selection.select(third, PathBuf::from("third.m4a"));
        selection.confirm(third).unwrap();
        assert_eq!(selection.confirmed, Some(PathBuf::from("third.m4a")));
    }
}
