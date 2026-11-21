use super::{Config, Gesture, LiveWheel, MotionPreset};
use serde_json::json;
use std::{fs, io::Write, path::PathBuf, time::Duration};

/// Diagnostic readback of the editor's optical render target. This never
/// captures UI, other windows, or the Windows desktop. The eight-second
/// sample covers one full ping-pong cycle of the shipped two-slide sequence.
pub(super) struct Capture {
    path: PathBuf,
    duration: Duration,
    origin: Option<Duration>,
    source_start: Option<[f64; 2]>,
    sample: u64,
    last: Duration,
    manifest: Option<fs::File>,
    configurations: Vec<serde_json::Value>,
    last_config: Option<(
        String,
        MotionPreset,
        u64,
        Option<f32>,
        u64,
        u32,
        Gesture,
        Option<f32>,
        u32,
    )>,
}

impl Capture {
    pub fn new(path: PathBuf, duration: Duration) -> Result<Self, String> {
        fs::create_dir(&path).map_err(|e| format!("Create new capture directory: {e}"))?;
        Ok(Self {
            path,
            duration,
            origin: None,
            source_start: None,
            sample: 0,
            last: Duration::ZERO,
            manifest: None,
            configurations: Vec::new(),
            last_config: None,
        })
    }

    // Use the same time value that the normal presenter submitted. A readback
    // can change throughput, so retain actual timestamps for variable-rate video.
    pub fn frame(
        &mut self,
        r: &LiveWheel,
        c: &Config,
        frame_time: Duration,
        applied_strokes: u64,
    ) -> Result<bool, String> {
        self.frame_at(
            r,
            c,
            frame_time,
            [frame_time.as_secs_f64(); 2],
            applied_strokes,
        )
    }

    pub fn frame_at(
        &mut self,
        r: &LiveWheel,
        c: &Config,
        frame_time: Duration,
        film_times: [f64; 2],
        applied_strokes: u64,
    ) -> Result<bool, String> {
        let elapsed = frame_time - *self.origin.get_or_insert(frame_time);
        let [rear_start, front_start] = *self.source_start.get_or_insert(film_times);
        let [rear_time, front_time] = film_times;
        let key = (
            c.wheel.clone(),
            c.motion,
            c.motion_take,
            c.focus,
            c.layer_id,
            c.slide as u32,
            c.gesture,
            c.tug_response,
            c.render_width,
        );
        if self.last_config.as_ref() != Some(&key) {
            self.configurations.push(
                json!({"t":elapsed.as_secs_f64(), "wheel":c.wheel, "motion":c.motion,
                "motionTake":c.motion_take, "fieldTime":front_time,
                "rearFieldTime":rear_time, "frontFieldTime":front_time,
                "focus":c.focus, "track":c.layer_id, "slide":c.slide, "gesture":c.gesture,
                "tugResponse":c.tug_response, "renderWidth":c.render_width}),
            );
            self.last_config = Some(key);
        }
        if elapsed >= self.duration && self.sample > 1 {
            if let Some(m) = self.manifest.as_mut() {
                writeln!(
                    m,
                    "duration {:.6}\nfile frame-{:04}.ppm\noption framerate 1000",
                    (elapsed - self.last).as_secs_f64(),
                    self.sample - 1
                )
                .map_err(|e| e.to_string())?;
            }
            let data = json!({"wheel": c.wheel, "focus": c.focus, "mouseTarget": c.slide,
                "track": c.layer_id, "adapter": r.adapter_name(), "size": r.preview_size(),
                "frames": self.sample, "successfulPresentsInWindow":self.sample,
                "gesture":c.gesture,"tugResponse":c.tug_response,
                "retention":"every successful present before the requested endpoint",
                "sourceStartSeconds":front_start, "rearSourceStartSeconds":rear_start,
                "frontSourceStartSeconds":front_start, "motion":c.motion, "motionTake":c.motion_take,
                "elapsedSeconds": elapsed.as_secs_f64(), "configurations":self.configurations,
                "appliedMouseStrokes":applied_strokes,
                "source": "active editor optical target immediately following successful presentation",
                "clock": "editor performance time, actual seconds relative to first captured frame"});
            fs::write(
                self.path.join("capture.json"),
                serde_json::to_vec_pretty(&data).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            return Ok(true);
        }
        let (w, h, rgba) = r.capture_rgba8()?;
        if self.manifest.is_none() {
            let mut m = fs::File::create_new(self.path.join("frames.ffconcat"))
                .map_err(|e| e.to_string())?;
            writeln!(m, "ffconcat version 1.0").map_err(|e| e.to_string())?;
            self.manifest = Some(m);
        }
        let m = self.manifest.as_mut().unwrap();
        if self.sample > 0 {
            writeln!(m, "duration {:.6}", (elapsed - self.last).as_secs_f64())
                .map_err(|e| e.to_string())?;
        }
        let name = format!("frame-{:04}.ppm", self.sample);
        let mut file = fs::File::create_new(self.path.join(&name)).map_err(|e| e.to_string())?;
        write!(file, "P6\n{w} {h}\n255\n").map_err(|e| e.to_string())?;
        let rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|p| p[..3].iter().copied())
            .collect();
        file.write_all(&rgb).map_err(|e| e.to_string())?;
        writeln!(m, "file {name}\noption framerate 1000").map_err(|e| e.to_string())?;
        self.last = elapsed;
        self.sample += 1;
        Ok(false)
    }
}
