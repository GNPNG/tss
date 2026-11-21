use super::{Host, MotionPreset, VisibleFilms};
use serde::Deserialize;
use tauri::State;
#[cfg(windows)]
use tss_live_wheel::production::DigitalFormat;
#[cfg(not(windows))]
#[path = "../../../live-wheel/src/production.rs"]
#[allow(dead_code)]
mod production;
#[cfg(not(windows))]
use production::DigitalFormat;

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
    #[serde(default)]
    pub(super) format: DigitalFormat,
}

#[tauri::command]
pub async fn preview_wheel_plates(
    host: State<'_, Host>,
    selection: Selection,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        use base64::Engine;
        return host
            .plate_preview(selection)
            .await
            .map(|v| base64::engine::general_purpose::STANDARD.encode(v));
    }
    #[cfg(not(windows))]
    {
        let _ = (host, selection);
        Err("Native wheel framing requires Windows".into())
    }
}
#[cfg(windows)]
impl Selection {
    pub(super) fn matches(&self, c: &super::Config) -> bool {
        c.layer_id == self.layer_id
            && c.wheel == self.wheel
            && c.motion == self.motion
            && c.motion_take == self.motion_take
            && c.visible_films == self.visible_films
            && c.focus == self.focus
    }
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
        selection.format.validate()?;
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
