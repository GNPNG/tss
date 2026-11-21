use std::sync::RwLock;

static FEATURES: RwLock<(f64, Vec<[f32; 4]>)> = RwLock::new((30., Vec::new()));

#[tauri::command]
pub async fn set_visual_audio_features(fps: f64, frames: Vec<[f32; 4]>) -> Result<(), String> {
    if !(0.0..=240.).contains(&fps)
        || fps == 0.
        || frames.iter().flatten().any(|x| !(0.0..=1.).contains(x))
    {
        return Err("Invalid visual audio features".into());
    }
    *FEATURES.write().map_err(|e| e.to_string())? = (fps, frames);
    Ok(())
}
#[cfg(windows)]
pub(super) fn at(time: f64) -> [f32; 4] {
    let data = FEATURES.read().unwrap_or_else(|e| e.into_inner());
    data.1
        .get(((data.0 * time.max(0.)).floor() as usize).min(data.1.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default()
}
