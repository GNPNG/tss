use super::{Host, MotionPreset, VisibleFilms};
use serde::Deserialize;
use tauri::State;
#[cfg(windows)]
use tss_live_wheel::production::{DigitalFormat, PrintFormat, PrintLayout};
#[cfg(not(windows))]
#[path = "../../../live-wheel/src/production.rs"]
#[allow(dead_code)]
mod production;
#[cfg(not(windows))]
use production::{DigitalFormat, PrintFormat, PrintLayout};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Selection {
    #[serde(default)]
    detail_scale: Option<f32>,
    pub(super) layer_id: u64,
    pub(super) wheel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) swatches: Option<[u32; 4]>,
    #[serde(default)]
    pub(super) motion: MotionPreset,
    #[serde(default)]
    pub(super) material_motion: super::MaterialMotion,
    #[serde(default)]
    pub(super) motion_take: u64,
    #[serde(default)]
    pub(super) visible_films: VisibleFilms,
    pub(super) focus: Option<f32>,
    #[serde(default)]
    pub(super) format: DigitalFormat,
    pub(super) print: Option<PrintFormat>,
}

#[tauri::command]
pub fn print_geometry(layout: PrintLayout) -> serde_json::Value {
    let mechanical = layout.mechanical();
    serde_json::json!({"spec":mechanical, "canvasMm":mechanical.canvas_mm(), "guide":mechanical.guide_svg()})
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
#[cfg(any(windows, test))]
impl Selection {
    pub(super) fn matches(&self, c: &super::Config) -> bool {
        !c.uses_simulation() && c.layer_id == self.layer_id && c.detail_scale == self.detail_scale
            && c.wheel == self.wheel
            && c.swatches == self.swatches
            && c.motion == self.motion
            && c.material_motion == self.material_motion
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
        let profile = if let Some(format) = selection.print {
            let Some(profile) = app
                .dialog()
                .file()
                .set_title(format!("Choose {}", format.stock.profile_name()))
                .add_filter("ECI print ICC profile", &["icc", "icm"])
                .blocking_pick_file()
            else {
                return Ok(None);
            };
            let path = profile.into_path().map_err(|e| e.to_string())?;
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 8_000_000 {
                return Err("Selected file exceeds the ECI profile size".into());
            }
            let mut icc = tss_live_wheel::PrintProfile::new(
                format.stock,
                std::fs::read(path).map_err(|e| e.to_string())?,
            )?;
            let choose_mask = |title: &str| -> Result<Option<Vec<u8>>, String> {
                let Some(path) = app
                    .dialog()
                    .file()
                    .set_title(title)
                    .add_filter("Full-canvas 1-bit grayscale stencil", &["png"])
                    .blocking_pick_file()
                else {
                    return Ok(None);
                };
                let path = path.into_path().map_err(|e| e.to_string())?;
                if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 8_000_000 {
                    return Err("Stencil exceeds 8 MB".into());
                }
                std::fs::read(path).map(Some).map_err(|e| e.to_string())
            };
            let spot = if let Some(g) = format.gilding {
                let Some(data) = choose_mask(&format!(
                    "Choose the registered sigil/title stencil for {}",
                    g.plate()
                ))?
                else {
                    return Ok(None);
                };
                Some(data)
            } else {
                None
            };
            let rich = if format.rich_black {
                let Some(data) = choose_mask("Choose the registered rich-black solids stencil")?
                else {
                    return Ok(None);
                };
                Some(data)
            } else {
                None
            };
            icc.set_masks(format, spot, rich)?;
            Some(icc)
        } else {
            None
        };
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
                .set_title("Choose a folder for the production artwork")
                .blocking_pick_folder()
            else {
                return Ok(None);
            };
            folder.into_path().map_err(|e| e.to_string())?
        };
        let result = host.plates(selection, folder, profile).await;
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

#[cfg(test)]
mod tests {
    use super::{super::Config, Selection};
    use serde_json::{from_value, json};

    #[test]
    fn plate_selection_matches_each_material_on_the_live_wheel() {
        let mut payload = json!({"layerId":2,"wheel":"violet-blue","slide":"rear",
            "rect":{"x":0,"y":0,"width":640,"height":360},"renderWidth":640});
        let legacy: Config = from_value(payload.clone()).unwrap();
        let old_plate: Selection = from_value(payload.clone()).unwrap();
        assert!(old_plate.matches(&legacy));

        payload["materialMotion"] = json!({"front":"pdms-20","rear":"pdms-1000"});
        let current: Config = from_value(payload.clone()).unwrap();
        assert!(!old_plate.matches(&current));
        let current_plate: Selection = from_value(payload.clone()).unwrap();
        assert!(current_plate.matches(&current));
        payload["materialMotion"]["front"] = json!("pdms-1000");
        assert!(!from_value::<Selection>(payload).unwrap().matches(&current));
    }
}
