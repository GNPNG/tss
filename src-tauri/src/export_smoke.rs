// The verification launcher explicitly grants exactly one audio and output path.
// Interactive picker grants and ordinary launches continue through their own paths.
use super::SelectedPaths;
use std::{io::Write, path::PathBuf};
use tauri::State;

const OPTION: &str = "--verify-wheel-export";

pub(crate) fn enabled() -> bool {
    std::env::args_os().any(|arg| arg == OPTION)
}

fn paths() -> Result<(PathBuf, PathBuf), String> {
    let mut args = std::env::args_os().skip_while(|v| v != OPTION).skip(1);
    let audio = args
        .next()
        .map(PathBuf::from)
        .ok_or("Missing verification audio path")?;
    let output = args
        .next()
        .map(PathBuf::from)
        .ok_or("Missing verification MP4 path")?;
    if !audio.is_absolute()
        || !output.is_absolute()
        || !output
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
    {
        return Err("Verification needs absolute audio and MP4 paths".into());
    }
    Ok((audio, output))
}

#[tauri::command]
pub(crate) fn select_wheel_export_fixture(
    state: State<'_, SelectedPaths>,
    window: tauri::WebviewWindow,
) -> Result<(u64, String, String, usize, bool), String> {
    let (audio, output) = paths()?;
    let audio = audio.canonicalize().map_err(|e| e.to_string())?;
    if !audio.is_file()
        || !audio
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("m4a") || e.eq_ignore_ascii_case("mp3"))
        || output.exists()
        || output.with_extension("verification.txt").exists()
        || output.with_extension("webview.json").exists()
        || !output.parent().is_some_and(|p| p.is_dir())
    {
        return Err("Verification needs an MP3/M4A input and a fresh output name".into());
    }
    let mut selected = state.audio.lock().map_err(|e| e.to_string())?;
    let id = selected.begin();
    let (_, path) = selected
        .select(id, audio)
        .ok_or("Verification selection superseded")?;
    *state.output.lock().map_err(|e| e.to_string())? = Some(output.clone());
    let displays = window
        .available_monitors()
        .map_err(|e| e.to_string())?
        .len();
    Ok((
        id,
        path,
        output.to_string_lossy().into_owned(),
        displays,
        std::env::args_os().any(|arg| arg == "--verify-pressure-export"),
    ))
}

#[tauri::command]
pub(crate) fn finish_wheel_export_verification(
    app: tauri::AppHandle,
    error: Option<String>,
    facts: String,
) -> Result<(), String> {
    let (_, output) = paths()?;
    let ok = error.is_none() && output.is_file();
    let message = error.unwrap_or_else(|| {
        if ok {
            "Export completed".into()
        } else {
            "Missing output".into()
        }
    });
    if facts.len() > 16384 {
        return Err("Unexpected verification result size".into());
    }
    let mut webview = std::fs::File::create_new(output.with_extension("webview.json"))
        .map_err(|e| e.to_string())?;
    webview
        .write_all(facts.as_bytes())
        .map_err(|e| e.to_string())?;
    drop(webview);
    let mut file = std::fs::File::create_new(output.with_extension("verification.txt"))
        .map_err(|e| e.to_string())?;
    file.write_all(message.as_bytes())
        .map_err(|e| e.to_string())?;
    drop(file);
    app.exit(if ok { 0 } else { 1 });
    Ok(())
}
