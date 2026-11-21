use std::sync::RwLock;

struct Analysis {
    fps: f64,
    frames: Vec<[f32; 4]>,
    bass_time: Vec<f64>,
}

static FEATURES: RwLock<Analysis> = RwLock::new(Analysis {
    fps: 30.,
    frames: Vec::new(),
    bass_time: Vec::new(),
});

impl Analysis {
    fn new(fps: f64, frames: Vec<[f32; 4]>) -> Result<Self, String> {
        if !(0.0..=240.).contains(&fps)
            || fps == 0.
            || frames.iter().flatten().any(|x| !(0.0..=1.).contains(x))
        {
            return Err("Invalid visual audio features".into());
        }
        let mut elapsed = 0.;
        let bass_time = frames
            .iter()
            .map(|sample| {
                let at_start = elapsed;
                elapsed += f64::from(sample[0]) / fps;
                at_start
            })
            .collect();
        Ok(Self {
            fps,
            frames,
            bass_time,
        })
    }

    fn at(&self, time: f64) -> [f32; 4] {
        if self.frames.is_empty() {
            return [0.; 4];
        }
        let sample = (time.max(0.) * self.fps).min(self.frames.len() as f64);
        let i = (sample.floor() as usize).min(self.frames.len() - 1);
        let [bass, mid, treble, _] = self.frames[i];
        let integrated = self.bass_time[i] + (sample - i as f64) * f64::from(bass) / self.fps;
        [bass, mid, treble, integrated as f32]
    }
}

#[tauri::command]
pub async fn set_visual_audio_features(fps: f64, frames: Vec<[f32; 4]>) -> Result<(), String> {
    let analysis = Analysis::new(fps, frames)?;
    *FEATURES.write().map_err(|e| e.to_string())? = analysis;
    Ok(())
}
#[cfg(any(windows, target_os = "macos"))]
pub(super) fn at(time: f64) -> [f32; 4] {
    FEATURES.read().unwrap_or_else(|e| e.into_inner()).at(time)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bass_acceleration_integrates_analysis_and_rewinds_exactly() {
        let track = Analysis::new(
            2.,
            vec![[0., 0.1, 0., 0.], [1., 0.4, 0., 0.], [0.5, 0.2, 0., 1.]],
        )
        .unwrap();
        for (t, sum) in [
            (0., 0.),
            (0.5, 0.),
            (1., 0.5),
            (1.25, 0.625),
            (20., 0.75),
            (0.75, 0.25),
        ] {
            assert_eq!(track.at(t)[3], sum);
        }
    }
}
