use super::{view, Config, Stats};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView};
use objc2_foundation::NSPoint;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
use tauri::WebviewWindow;
use tss_live_wheel::LiveWheel;

/// Three spaced samples of the right-eye texture just submitted to the TV.
/// Capturing this texture does not read the desktop or run another render.
pub(super) struct EyeCapture {
    directory: PathBuf,
    visible_since: Option<Instant>,
    previous_visible: Duration,
    count: usize,
}

impl EyeCapture {
    pub(super) fn from_environment() -> Option<Self> {
        let path = std::env::var_os("TSS_LIVE_STEREO_CAPTURE_DIR").filter(|p| !p.is_empty())?;
        Some(Self {
            directory: path.into(),
            visible_since: None,
            previous_visible: Duration::ZERO,
            count: 0,
        })
    }

    pub(super) fn visibility(&mut self, visible: bool, now: Instant) {
        if visible {
            self.visible_since.get_or_insert(now);
        } else if let Some(since) = self.visible_since.take() {
            self.previous_visible += now.duration_since(since);
        }
    }

    pub(super) fn on_present(
        &mut self,
        wheel: &LiveWheel,
        config: &Config,
        stats: &Stats,
        now: Instant,
    ) -> Result<(), String> {
        const TIMES: [u64; 3] = [4, 60, 62];
        let Some(&target) = TIMES.get(self.count) else {
            return Ok(());
        };
        let visible_time =
            self.previous_visible + now.duration_since(*self.visible_since.get_or_insert(now));
        if visible_time < Duration::from_secs(target) {
            return Ok(());
        }
        std::fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        let base = format!("right-eye-{:02}", self.count + 1);
        let frame = File::create_new(self.directory.join(format!("{base}.png")))
            .map_err(|e| e.to_string())?;
        let (width, height, rgba) = wheel.capture_rgba8()?;
        let mut encoder = png::Encoder::new(frame, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        let metadata = File::create_new(self.directory.join(format!("{base}.json")))
            .map_err(|e| e.to_string())?;
        let mut details = serde_json::to_value(stats).map_err(|e| e.to_string())?;
        details["captureVisibleSeconds"] = serde_json::json!(visible_time.as_secs_f64());
        details["eye"] = serde_json::json!("right");
        details["wheel"] = serde_json::json!(config.wheel);
        details["simulation"] = serde_json::json!(config.simulation);
        details["swatches"] = serde_json::json!(config.swatches);
        serde_json::to_writer_pretty(metadata, &details).map_err(|e| e.to_string())?;
        self.count += 1;
        Ok(())
    }
}

// Exercise only the editor's own window. No global mouse events or cursor movement.
pub(super) fn inspect(window: &WebviewWindow, dir: &Path) -> Result<isize, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    window
        .with_webview(move |webview| {
            // Tauri holds this WKWebView on the Cocoa main thread.
            let parent = unsafe { &*webview.inner().cast::<NSView>() };
            let _ = tx.send(inspect_view(parent));
        })
        .map_err(|e| e.to_string())?;
    let (number, info) = rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())??;
    serde_json::to_writer_pretty(
        File::create_new(dir.join("view.json")).map_err(|e| e.to_string())?,
        &info,
    )
    .map_err(|e| e.to_string())?;
    Ok(number)
}

fn inspect_view(parent: &NSView) -> Result<(isize, serde_json::Value), String> {
    let child = view::child(parent).ok_or("Preview child is absent")?;
    let window = child.window().ok_or("Preview window is absent")?;
    let content = window.contentView().ok_or("Missing window content")?;
    let hit = |position| {
        // The live NSWindow retains the content hierarchy throughout this main-thread callback.
        let point = unsafe { content.superview() }.map_or(position, |parent| {
            parent.convertPoint_fromView(position, None)
        });
        content.hitTest(point)
    };
    let number = window.windowNumber();
    let bounds = child.bounds().size;
    let frame = child.frame();
    let in_window = child.convertRect_toView(child.bounds(), None);
    let window_height = window.frame().size.height;
    let center =
        child.convertPoint_toView(NSPoint::new(bounds.width / 2., bounds.height / 2.), None);
    let outside = parent.convertPoint_toView(NSPoint::new(4., 4.), None);
    let center_hit = hit(center).is_some_and(|v| v == child);
    let outside_hit = hit(outside).is_some_and(|v| v != child);
    if center_hit {
        for (i, (kind, x, y)) in [
            (NSEventType::LeftMouseDown, 0.45, 0.55),
            (NSEventType::LeftMouseDragged, 0.58, 0.48),
            (NSEventType::LeftMouseUp, 0.58, 0.48),
        ]
        .into_iter()
        .enumerate()
        {
            let point =
                child.convertPoint_toView(NSPoint::new(bounds.width * x, bounds.height * y), None);
            let event=NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                        kind,point,NSEventModifierFlags::empty(),1.+i as f64/100.,number,None,i as isize+1,1,1.).ok_or("Cannot create local AppKit mouse event")?;
            window.sendEvent(&event);
        }
    }
    Ok((
        number,
        serde_json::json!({
            "windowNumber":number,"backingScale":window.backingScaleFactor(),"parentFlipped":parent.isFlipped(),
            "viewPoints":{"x":frame.origin.x,"y":frame.origin.y,"width":frame.size.width,"height":frame.size.height},
            "windowTopLeftPoints":{"x":in_window.origin.x,"y":window_height-(in_window.origin.y+in_window.size.height),"width":in_window.size.width,"height":in_window.size.height},
            "centerHitNativeWheel":center_hit,"outsideHitEditor":outside_hit,
            "localMouseEvents":if center_hit {3} else {0}
        }),
    ))
}

pub(super) fn capture(
    dir: &Path,
    r: &LiveWheel,
    stats: &Stats,
    window: isize,
) -> Result<(), String> {
    let (w, h, rgba) = r.capture_rgba8()?;
    let file = File::create_new(dir.join("wheel.png")).map_err(|e| e.to_string())?;
    let mut png = png::Encoder::new(file, w, h);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    let mut writer = png.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(
        File::create_new(dir.join("preview.json")).map_err(|e| e.to_string())?,
        stats,
    )
    .map_err(|e| e.to_string())?;
    let screenshot = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", "-l", &window.to_string()])
        .arg(dir.join("editor.png"))
        .output()
        .map_err(|e| e.to_string())?;
    if !screenshot.status.success() {
        eprintln!(
            "Window screenshot unavailable: {}",
            String::from_utf8_lossy(&screenshot.stderr)
        );
    }
    Ok(())
}
