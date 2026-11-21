use super::SelectedPaths;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

const MAX_PROJECT: usize = 64 * 1024 * 1024;
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
struct AudioRef {
    name: String,
    sha256: String,
}

#[derive(Deserialize)]
struct ProjectHeader {
    format: String,
    version: u32,
    audio: Option<AudioRef>,
}

fn valid_digest(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn read_project(path: &Path) -> Result<String, String> {
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("tss"))
    {
        return Err("Choose a .tss project file.".into());
    }
    if !fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
        return Err("The selected project is not a regular file.".into());
    }
    let mut text = String::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_PROJECT as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > MAX_PROJECT {
        return Err("Project exceeds 64 MiB.".into());
    }
    Ok(text)
}

// Place the temporary file on the destination volume. A failed write or replacement
// leaves the previous project intact. create_new also protects simultaneous saves.
fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let (temporary, mut file) = loop {
        let serial = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_file_name(format!(".tss-{}-{serial}.tmp", std::process::id()));
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn same_audio(path: &Path, expected: &str) -> bool {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3") || e.eq_ignore_ascii_case("m4a"))
        || !fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
    {
        return false;
    }
    digest_file(path).is_ok_and(|digest| digest == expected)
}

fn digest_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn locate_audio(directory: &Path, links: &Path, reference: &AudioRef) -> Option<PathBuf> {
    if !valid_digest(&reference.sha256) {
        return None;
    }
    let name = Path::new(&reference.name);
    if !matches!(
        name.components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(_)]
    ) || reference.name.contains(['/', '\\'])
    {
        return None;
    }
    let sibling = directory.join(name);
    if same_audio(&sibling, &reference.sha256) {
        return Some(sibling);
    }
    // Only a prior native file-picker grant can populate this machine-local link.
    let remembered: PathBuf =
        serde_json::from_slice(&fs::read(links.join(&reference.sha256)).ok()?).ok()?;
    same_audio(&remembered, &reference.sha256).then_some(remembered)
}

fn links_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("audio-links"))
}

#[tauri::command]
pub async fn open_project(
    app: tauri::AppHandle,
    paths: State<'_, SelectedPaths>,
) -> Result<Option<(String, String, Option<(u64, String)>)>, String> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Sacred Swirls project", &["tss"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let links = links_directory(&app)?;
    let (document, audio, name) = tauri::async_runtime::spawn_blocking(move || {
        let document = read_project(&path)?;
        let header: ProjectHeader = serde_json::from_str(&document).map_err(|e| e.to_string())?;
        let audio = header
            .audio
            .filter(|_| header.format == "the-sacred-swirls" && header.version == 1)
            .and_then(|r| locate_audio(path.parent()?, &links, &r))
            .and_then(|p| fs::canonicalize(p).ok());
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Ok::<_, String>((document, audio, name))
    })
    .await
    .map_err(|e| e.to_string())??;
    let selection = if let Some(audio) = audio {
        let mut state = paths.audio.lock().map_err(|e| e.to_string())?;
        let id = state.begin();
        state.select(id, audio)
    } else {
        None
    };
    Ok(Some((name, document, selection)))
}

#[tauri::command]
pub async fn project_audio_digest(
    paths: State<'_, SelectedPaths>,
    selection_id: u64,
) -> Result<String, String> {
    let path = paths
        .audio
        .lock()
        .map_err(|e| e.to_string())?
        .pending_path(selection_id)?;
    tauri::async_runtime::spawn_blocking(move || digest_file(&path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_project_audio(paths: State<'_, SelectedPaths>) -> Result<(), String> {
    let mut audio = paths.audio.lock().map_err(|e| e.to_string())?;
    audio.begin();
    audio.confirmed = None;
    audio.confirmed_id = None;
    Ok(())
}

#[tauri::command]
pub async fn remember_project_audio(
    app: tauri::AppHandle,
    paths: State<'_, SelectedPaths>,
    selection_id: u64,
    sha256: String,
) -> Result<(), String> {
    if !valid_digest(&sha256) {
        return Err("Invalid audio fingerprint.".into());
    }
    let path = {
        let state = paths.audio.lock().map_err(|e| e.to_string())?;
        if state.confirmed_id != Some(selection_id) {
            return Err("Audio selection changed.".into());
        }
        state.confirmed.clone().ok_or("Choose audio first.")?
    };
    let links = links_directory(&app)?;
    fs::create_dir_all(&links).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&path).map_err(|e| e.to_string())?;
    atomic_write(&links.join(sha256), &bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_project(
    app: tauri::AppHandle,
    document: String,
    suggested_name: String,
) -> Result<Option<String>, String> {
    if document.len() > MAX_PROJECT {
        return Err("Project exceeds 64 MiB.".into());
    }
    let basename = Path::new(&suggested_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("session.tss");
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Sacred Swirls project", &["tss"])
        .set_file_name(basename)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let mut path = file.into_path().map_err(|e| e.to_string())?;
    if path.extension().is_none() {
        path.set_extension("tss");
        if path.exists() {
            return Err("Select the existing .tss file in the save dialog to replace it.".into());
        }
    }
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("tss"))
    {
        return Err("Choose a filename ending in .tss.".into());
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    tauri::async_runtime::spawn_blocking(move || atomic_write(&path, document.as_bytes()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_round_trip_and_relink() {
        let root = std::env::temp_dir().join(format!(
            "tss-project-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let links = root.join("links");
        fs::create_dir(&links).unwrap();
        let document = root.join("take.tss");
        atomic_write(&document, b"first").unwrap();
        atomic_write(&document, b"second").unwrap();
        assert_eq!(read_project(&document).unwrap(), "second");
        let source = root.join("song.mp3");
        fs::write(&source, b"song bytes").unwrap();
        let hash = format!("{:x}", Sha256::digest(b"song bytes"));
        let mut reference = AudioRef {
            name: "song.mp3".into(),
            sha256: hash.clone(),
        };
        assert_eq!(
            locate_audio(&root, &links, &reference),
            Some(source.clone())
        );
        let moved = root.join("renamed.mp3");
        fs::rename(&source, &moved).unwrap();
        fs::write(&source, b"different song").unwrap();
        assert_eq!(locate_audio(&root, &links, &reference), None);
        atomic_write(&links.join(&hash), &serde_json::to_vec(&moved).unwrap()).unwrap();
        assert_eq!(locate_audio(&root, &links, &reference), Some(moved));
        reference.name = "../song.mp3".into();
        assert_eq!(locate_audio(&root, &links, &reference), None);
        fs::remove_dir_all(root).unwrap();
    }
}
