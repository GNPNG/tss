use super::{Host, MotionPreset, VisibleFilms};
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Selection {
    pub(super) layer_id: u64,
    pub(super) wheel: String,
    pub(super) motion: MotionPreset,
    pub(super) motion_take: u64,
    pub(super) visible_films: VisibleFilms,
    pub(super) focus: Option<f32>,
}

#[tauri::command]
pub async fn export_native_wheel_plates(
    app: tauri::AppHandle,
    host: State<'_, Host>,
    selection: Selection,
) -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        use tauri_plugin_dialog::DialogExt;
        let fixture = super::windows::plate_verification();
        let folder = if let Some(dir) = &fixture {
            if !dir.is_absolute() || !dir.is_dir() {
                app.exit(1);
                return Err(
                    "Plate verification requires an existing absolute output folder".into(),
                );
            }
            dir.clone()
        } else {
            let Some(folder) = app
                .dialog()
                .file()
                .set_title("Choose a folder for the 4K wheel plates")
                .blocking_pick_folder()
            else {
                return Ok(None);
            };
            folder.into_path().map_err(|e| e.to_string())?
        };
        let result = host.plates(selection, folder).await;
        if fixture.is_some() {
            if let Err(e) = &result {
                eprintln!("Plate verification failed: {e}");
            }
            app.exit(if result.is_ok() { 0 } else { 1 });
        }
        result.map(|p| Some(p.to_string_lossy().into_owned()))
    }
    #[cfg(not(windows))]
    {
        let _ = (app, host, selection);
        Err("Live wheel plate export requires the Windows native editor".into())
    }
}
