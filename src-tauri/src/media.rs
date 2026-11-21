//! Prepared visual clips use independently decodable PNG samples on their original clock.
mod prepare;

use prepare::{Index, Track};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

type MediaError = String;
type MediaResult<T> = Result<T, MediaError>;
fn ioerr(e: impl std::fmt::Display) -> MediaError {
    e.to_string()
}

#[derive(Clone)]
pub struct DecodedFrame {
    pub id: u64,
    pub pts_us: i64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    id: u64,
    sha256: String,
    name: String,
    duration_us: i64,
    width: u32,
    height: u32,
    still: bool,
}

struct Asset {
    grant: Grant,
    dir: PathBuf,
    index: Index,
}

impl Asset {
    fn track(&self, preview: bool) -> &Track {
        if preview {
            self.index.proxy.as_ref().unwrap_or(&self.index.master)
        } else {
            &self.index.master
        }
    }
}

type Key = (u64, bool, usize);
#[derive(Default)]
struct Store {
    by_hash: HashMap<String, u64>,
    grants: HashMap<u64, Arc<Asset>>,
    frames: VecDeque<(Key, DecodedFrame)>,
    bytes: usize,
}
static STORE: LazyLock<Mutex<Store>> = LazyLock::new(Default::default);
static PREPARE_LOCK: Mutex<()> = Mutex::new(());
const MEMORY_BYTES: usize = 192 * 1024 * 1024;
static PREFETCH: LazyLock<mpsc::SyncSender<(Arc<Asset>, bool, usize)>> = LazyLock::new(|| {
    let (send, recv) = mpsc::sync_channel::<(Arc<Asset>, bool, usize)>(3);
    std::thread::spawn(move || {
        while let Ok((asset, preview, index)) = recv.recv() {
            let _ = decode_cached(&asset, preview, index);
        }
    });
    send
});

static REC709_TO_SRGB: LazyLock<[u8; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let code = i as f64 / 255.0;
        let linear = if code < 0.081 {
            code / 4.5
        } else {
            ((code + 0.099) / 1.099).powf(1.0 / 0.45)
        };
        let s = if linear <= 0.0031308 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round().clamp(0.0, 255.0) as u8
    })
});

fn cached(store: &mut Store, key: Key) -> Option<DecodedFrame> {
    let i = store.frames.iter().position(|(k, _)| *k == key)?;
    let entry = store.frames.remove(i)?;
    let frame = entry.1.clone();
    store.frames.push_back(entry);
    Some(frame)
}

fn decode_cached(asset: &Asset, preview: bool, index: usize) -> MediaResult<DecodedFrame> {
    let key = (asset.grant.id, preview, index);
    {
        let mut store = STORE.lock().map_err(ioerr)?;
        if let Some(f) = cached(&mut store, key) {
            return Ok(f);
        }
    }
    let track = asset.track(preview);
    let packet = &track.samples[index];
    let file = File::open(
        asset
            .dir
            .join(if preview { "preview.mov" } else { "master.mov" }),
    )
    .map_err(ioerr)?;
    let mut file = BufReader::new(file);
    file.seek(SeekFrom::Start(packet.pos)).map_err(ioerr)?;
    let mut compressed = vec![0; packet.size as usize];
    file.read_exact(&mut compressed).map_err(ioerr)?;
    let mut reader = png::Decoder::new(Cursor::new(compressed))
        .read_info()
        .map_err(ioerr)?;
    let header = reader.info();
    if header.width != track.width
        || header.height != track.height
        || header.color_type != png::ColorType::Rgba
        || header.bit_depth != png::BitDepth::Eight
    {
        return Err("Prepared PNG sample disagrees with its frame index.".into());
    }
    let mut pixels = vec![0; track.width as usize * track.height as usize * 4];
    reader.next_frame(&mut pixels).map_err(ioerr)?;
    if asset.index.rec709_transfer {
        for px in pixels.chunks_exact_mut(4) {
            for c in &mut px[..3] {
                *c = REC709_TO_SRGB[*c as usize];
            }
        }
    }
    let frame = DecodedFrame {
        id: asset.grant.id,
        pts_us: packet.time_us,
        width: track.width,
        height: track.height,
        rgba: pixels.into(),
    };
    let mut store = STORE.lock().map_err(ioerr)?;
    if let Some(existing) = cached(&mut store, key) {
        return Ok(existing);
    }
    while store.bytes + frame.rgba.len() > MEMORY_BYTES {
        let Some((_, evicted)) = store.frames.pop_front() else {
            break;
        };
        store.bytes -= evicted.rgba.len();
    }
    store.bytes += frame.rgba.len();
    store.frames.push_back((key, frame.clone()));
    Ok(frame)
}

fn request(id: u64, us: i64, preview: bool) -> MediaResult<DecodedFrame> {
    let asset = STORE
        .lock()
        .map_err(ioerr)?
        .grants
        .get(&id)
        .cloned()
        .ok_or("Select or relink this visual media first.")?;
    let use_proxy = preview && asset.index.proxy.is_some();
    let track = asset.track(use_proxy);
    let index = track
        .samples
        .partition_point(|p| p.time_us <= us)
        .saturating_sub(1);
    let frame = decode_cached(&asset, use_proxy, index)?;
    if index + 1 < track.samples.len() {
        let _ = PREFETCH.try_send((asset, use_proxy, index + 1));
    }
    Ok(frame)
}

pub fn frame_at(asset_id: u64, source_us: i64) -> MediaResult<DecodedFrame> {
    request(asset_id, source_us, false)
}
pub fn frame_for_preview(asset_id: u64, source_us: i64) -> MediaResult<DecodedFrame> {
    request(asset_id, source_us, true)
}

fn register(dir: PathBuf, idx: Index, name: String, sha256: String) -> MediaResult<Grant> {
    let mut store = STORE.lock().map_err(ioerr)?;
    if let Some(existing) = store
        .by_hash
        .get(&sha256)
        .and_then(|id| store.grants.get(id))
    {
        let mut grant = existing.grant.clone();
        grant.name = name;
        return Ok(grant);
    }
    let src = &idx.master;
    let last = src.samples.last().ok_or("Prepared media has no samples.")?;
    let id = store.grants.len() as u64 + 1;
    let grant = Grant {
        id,
        sha256: sha256.clone(),
        name,
        duration_us: last.time_us + last.duration_us,
        width: src.width,
        height: src.height,
        still: src.samples.len() == 1,
    };
    store.by_hash.insert(sha256, id);
    store.grants.insert(
        id,
        Arc::new(Asset {
            grant: grant.clone(),
            dir,
            index: idx,
        }),
    );
    Ok(grant)
}

fn cache(app: &tauri::AppHandle) -> MediaResult<PathBuf> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(ioerr)?
        .join("visual-media-v1"))
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn import(root: &Path, source: &Path, expected: Option<&str>) -> MediaResult<Grant> {
    let hash = crate::project::digest_file(source).map_err(ioerr)?;
    if expected.is_some_and(|expected| expected != hash) {
        return Err("The selected visual file does not match the project's SHA-256.".into());
    }
    let name = source
        .file_name()
        .ok_or("Missing media filename.")?
        .to_string_lossy()
        .into_owned();
    let _lock = PREPARE_LOCK.lock().map_err(ioerr)?;
    let (dir, index) = prepare::prepare(source, root, &hash)?;
    register(dir, index, name, hash)
}
async fn pick(app: tauri::AppHandle, expected: Option<String>) -> MediaResult<Option<Grant>> {
    let root = cache(&app)?;
    let selected = app
        .dialog()
        .file()
        .add_filter(
            "Visual media",
            &[
                "png", "jpg", "jpeg", "gif", "apng", "webp", "tif", "tiff", "bmp", "mp4", "m4v",
                "mov", "webm", "mkv", "avi",
            ],
        )
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let source = selected.into_path().map_err(ioerr)?;
    tauri::async_runtime::spawn_blocking(move || import(&root, &source, expected.as_deref()))
        .await
        .map_err(ioerr)?
        .map(Some)
}
#[tauri::command]
pub async fn choose_visual_media(app: tauri::AppHandle) -> MediaResult<Option<Grant>> {
    pick(app, None).await
}
#[tauri::command]
pub async fn relink_visual_media(
    app: tauri::AppHandle,
    sha256: String,
) -> MediaResult<Option<Grant>> {
    if !valid_hash(&sha256) {
        return Err("Invalid visual fingerprint.".into());
    }
    pick(app, Some(sha256)).await
}
#[tauri::command]
pub async fn resolve_visual_media(
    app: tauri::AppHandle,
    sha256: String,
    name: String,
) -> MediaResult<Option<Grant>> {
    if !valid_hash(&sha256) {
        return Err("Invalid visual fingerprint.".into());
    }
    let root = cache(&app)?;
    let sibling = crate::project::project_media_sibling(&app, &name);
    tauri::async_runtime::spawn_blocking(move || {
        let dir = root.join(&sha256);
        if dir.is_dir() {
            let index = prepare::load(&dir)?;
            return register(dir, index, name, sha256).map(Some);
        }
        if let Some(path) = sibling {
            if crate::project::digest_file(&path).map_err(ioerr)? == sha256 {
                return import(&root, &path, Some(&sha256)).map(Some);
            }
        }
        Ok(None)
    })
    .await
    .map_err(ioerr)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translucent_proxy_edges_keep_the_visible_dye_colour() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("edge.png");
        let width = 1920;
        let height = 16;
        let mut encoder = png::Encoder::new(File::create(&path).unwrap(), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let pixels = [255, 0, 0, 0, 0, 255, 255, 255].repeat((width * height / 2) as usize);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        let grant = import(&temp.path().join("cache"), &path, None).unwrap();
        let proxy = frame_for_preview(grant.id, 0).unwrap();
        assert_eq!((proxy.width, proxy.height), (960, 8));
        assert!(proxy
            .rgba
            .chunks_exact(4)
            .all(|p| p[0] <= 1 && p[1] >= 254 && p[2] >= 254 && (127..=128).contains(&p[3])));
        assert_eq!(&*frame_at(grant.id, 0).unwrap().rgba, pixels);
    }

    #[test]
    fn png_media_preserves_alpha_pixels_and_portable_identity() {
        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("violet & cyan.png");
        let pixels = [235, 0, 105, 0, 10, 90, 240, 211];
        let mut enc = png::Encoder::new(File::create(&img).unwrap(), 2, 1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        let root = dir.path().join("cache");
        let g = import(&root, &img, None).unwrap();
        assert!(g.still);
        let frame = frame_at(g.id, 9999999).unwrap();
        assert_eq!(&*frame.rgba, pixels);
        assert_eq!(
            (frame.id, frame.pts_us, frame.width, frame.height),
            (g.id, 0, 2, 1)
        );
        assert!(Arc::ptr_eq(
            &frame.rgba,
            &frame_for_preview(g.id, 0).unwrap().rgba
        ));
        assert_eq!(import(&root, &img, Some(&g.sha256)).unwrap().id, g.id);
        assert!(import(&root, &img, Some(&"a".repeat(64))).is_err());
    }
}

#[cfg(test)]
mod animated_tests {
    use super::*;
    #[test]
    fn seek_uses_original_vfr_intervals_in_both_master_and_preview() {
        let tmp = tempfile::tempdir().unwrap();
        let colors = [[200, 40, 17, 0], [14, 91, 239, 127], [22, 175, 44, 255]];
        for (i, rgba) in colors.iter().enumerate() {
            let p = tmp.path().join(format!("{i}.png"));
            let mut enc = png::Encoder::new(File::create(p).unwrap(), 1200, 300);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(&rgba.repeat(1200 * 300))
                .unwrap();
        }
        std::fs::write(
            tmp.path().join("sources.txt"),
            "file '0.png'\nduration 0.2\nfile '1.png'\nduration 0.4\nfile '2.png'\n",
        )
        .unwrap();
        let src = tmp.path().join("irregular.mov");
        let o = std::process::Command::new("ffmpeg")
            .current_dir(tmp.path())
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
                "sources.txt",
                "-fps_mode",
                "passthrough",
                "-c:v",
                "png",
                "-pix_fmt",
                "rgba",
                "-threads",
                "1",
            ])
            .arg(&src)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let g = import(&tmp.path().join("cache"), &src, None).unwrap();
        assert!(!g.still);
        assert_eq!(g.duration_us, 640000);
        for (us, i, pts) in [
            (0, 0, 0),
            (600_000, 2, 600_000),
            (599_999, 1, 200_000),
            (200_000, 1, 200_000),
            (199_999, 0, 0),
        ] {
            let full = frame_at(g.id, us).unwrap();
            let proxy = frame_for_preview(g.id, us).unwrap();
            assert_eq!(
                (full.width, full.height, proxy.width, proxy.height),
                (1200, 300, 960, 240)
            );
            assert_eq!((full.pts_us, proxy.pts_us), (pts, pts));
            assert_eq!(full.rgba[0..4], colors[i]);
            // The RGB values of fully transparent output pixels are immaterial after resampling.
            assert_eq!(proxy.rgba[3], colors[i][3]);
            if colors[i][3] > 0 {
                assert!(proxy.rgba[..3]
                    .iter()
                    .zip(colors[i])
                    .all(|(c, w)| c.abs_diff(w) <= 2));
            }
        }
        let gif = tmp.path().join("animated.gif");
        let o = std::process::Command::new("ffmpeg")
            .args(["-nostdin", "-loglevel", "error", "-i"])
            .arg(&src)
            .args(["-vf", "scale=120:30", "-loop", "0"])
            .arg(&gif)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let g = import(&tmp.path().join("cache"), &gif, None).unwrap();
        assert!(!g.still);
        assert_ne!(
            frame_at(g.id, 0).unwrap().rgba,
            frame_at(g.id, g.duration_us - 1).unwrap().rgba
        );
    }
}
