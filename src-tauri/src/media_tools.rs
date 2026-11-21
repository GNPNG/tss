//! Discover the external media programs used by artwork preparation and MP4 export.
use process_wrap::std::CommandWrap;
use serde::Serialize;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

static RESOLVED: Mutex<Option<Report>> = Mutex::new(None);
static INSTALL: Mutex<()> = Mutex::new(());
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(900);

#[derive(Clone, Copy)]
pub(crate) enum Binary {
    Ffmpeg,
    Ffprobe,
}

impl Binary {
    fn name(self) -> &'static str {
        match self {
            Self::Ffmpeg => "ffmpeg",
            Self::Ffprobe => "ffprobe",
        }
    }
}

#[derive(Clone, Copy)]
enum Package {
    Brew,
    Winget,
    Unmanaged,
}
impl Package {
    fn local() -> Self {
        if cfg!(target_os = "macos") {
            Self::Brew
        } else if cfg!(windows) {
            Self::Winget
        } else {
            Self::Unmanaged
        }
    }
    fn spec(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Self::Brew => Some(("brew", &["install", "ffmpeg"])),
            Self::Winget => Some((
                "winget",
                &[
                    "install",
                    "--id",
                    "Gyan.FFmpeg",
                    "--exact",
                    "--source",
                    "winget",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                    "--disable-interactivity",
                ],
            )),
            Self::Unmanaged => None,
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Brew => "Homebrew is required for automatic installation on macOS.",
            Self::Winget => "Install App Installer from Microsoft to enable WinGet.",
            Self::Unmanaged => {
                "Install FFmpeg and FFprobe with the system package manager, then recheck."
            }
        }
    }
}

#[derive(Clone, Serialize)]
struct ToolStatus {
    name: &'static str,
    path: Option<PathBuf>,
    version: Option<String>,
    issues: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    tools: Vec<ToolStatus>,
    ready: bool,
    install_command: Option<String>,
    manager_available: bool,
    manager_help: &'static str,
}

fn find(dirs: &[PathBuf], name: &str) -> Vec<PathBuf> {
    let exe = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let mut paths = Vec::new();
    for dir in dirs.iter().filter(|d| d.is_absolute()) {
        let path = dir.join(&exe);
        // WinGet's Links folder can enumerate a valid NTFS link that CreateProcess
        // cannot launch. Start the installed codec executable at the link target.
        #[cfg(windows)]
        if matches!(name, "ffmpeg" | "ffprobe") {
            if let Some(target) = linked_executable(&path) {
                if !paths.contains(&target) {
                    paths.push(target);
                }
            }
        }
        if !paths.contains(&path) && (path.is_file() || cfg!(windows) && path.is_symlink()) {
            paths.push(path);
        }
    }
    paths
}

#[cfg(any(windows, all(test, unix)))]
fn linked_executable(entry: &Path) -> Option<PathBuf> {
    let target = std::fs::read_link(entry).ok()?;
    let target = entry.parent()?.join(target);
    target.is_file().then_some(target)
}

fn run(path: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    // Long downloads can fill a pipe even while the UI thread remains free.
    // Spool the process output and retain only its final MiB for inspection.
    let err = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut log = tempfile::tempfile().map_err(err)?;
    let mut cmd = Command::new(path);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(err)?)
        .stderr(log.try_clone().map_err(err)?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW for CLI tools launched from the GUI.
    }
    let mut cmd = CommandWrap::from(cmd);
    #[cfg(unix)]
    cmd.wrap(process_wrap::std::ProcessGroup::leader());
    #[cfg(windows)]
    cmd.wrap(process_wrap::std::JobObject);
    let mut child = cmd.spawn().map_err(err)?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(25)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result
                    .err()
                    .map(err)
                    .unwrap_or_else(|| format!("{} timed out.", path.display())));
            }
        }
    };
    let at = log
        .metadata()
        .map_err(err)?
        .len()
        .saturating_sub(1024 * 1024);
    log.seek(SeekFrom::Start(at)).map_err(err)?;
    let mut bytes = Vec::new();
    log.read_to_end(&mut bytes).map_err(err)?;
    let text = String::from_utf8_lossy(&bytes);
    if status.success() {
        Ok(text.into_owned())
    } else {
        let chars: Vec<_> = text.trim().chars().collect();
        let last: String = chars[chars.len().saturating_sub(1500)..].iter().collect();
        Err(format!("{} exited with {status}. {last}", path.display()))
    }
}

type Runner<'a> = dyn Fn(&Path, &[&str], Duration) -> Result<String, String> + 'a;

fn inspect_binary(dirs: &[PathBuf], binary: Binary, runner: &Runner<'_>) -> ToolStatus {
    let name = binary.name();
    let mut first = None;
    for path in find(dirs, name) {
        let mut tool = ToolStatus {
            name,
            path: Some(path.clone()),
            version: None,
            issues: vec![],
        };
        let result = match binary {
            Binary::Ffprobe => runner(
                &path,
                &["-v", "error", "-show_program_version", "-of", "json"],
                PROBE_TIMEOUT,
            )
            .and_then(|s| {
                let json: serde_json::Value = serde_json::from_str(&s)
                    .map_err(|e| format!("FFprobe JSON output failed: {e}"))?;
                json["program_version"]["version"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| "FFprobe did not report a program version.".to_string())
            }),
            Binary::Ffmpeg => runner(&path, &["-version"], PROBE_TIMEOUT).and_then(|s| {
                s.lines()
                    .find_map(|l| l.strip_prefix("ffmpeg version "))
                    .and_then(|v| v.split_whitespace().next())
                    .map(str::to_owned)
                    .ok_or_else(|| "FFmpeg did not report a program version.".to_string())
            }),
        };
        match result {
            Err(e) => tool.issues.push(e),
            Ok(version) => {
                tool.version = Some(version);
                if matches!(binary, Binary::Ffmpeg) {
                    for (flag, label, required) in [
                        ("-encoders", "encoders", &["libx264", "aac", "png"][..]),
                        (
                            "-decoders",
                            "decoders",
                            &["png", "rawvideo", "libvpx", "libvpx-vp9"][..],
                        ),
                        (
                            "-filters",
                            "filters",
                            &[
                                "colorspace",
                                "scale",
                                "format",
                                "setpts",
                                "setsar",
                                "premultiply",
                                "unpremultiply",
                            ][..],
                        ),
                        ("-muxers", "muxers", &["mov", "mp4"][..]),
                    ] {
                        match runner(&path, &["-hide_banner", flag], PROBE_TIMEOUT) {
                            Err(e) => tool.issues.push(e),
                            Ok(s) => {
                                let names: std::collections::HashSet<_> = s
                                    .lines()
                                    .filter_map(|line| line.split_whitespace().nth(1))
                                    .flat_map(|field| field.split(','))
                                    .collect();
                                let missing: Vec<_> = required
                                    .iter()
                                    .copied()
                                    .filter(|v| !names.contains(v))
                                    .collect();
                                if !missing.is_empty() {
                                    tool.issues
                                        .push(format!("Missing {label}: {}", missing.join(", ")));
                                }
                            }
                        }
                    }
                }
            }
        }
        if matches!(binary, Binary::Ffmpeg) && tool.issues.is_empty() {
            // Parse the timestamp options used to preserve variable-frame-rate artwork.
            // Empty local stdin and a zero-frame null sink produce no output media.
            let args = [
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgba",
                "-video_size",
                "4x4",
                "-framerate",
                "24",
                "-i",
                "pipe:0",
                "-frames:v",
                "0",
                "-fps_mode",
                "passthrough",
                "-enc_time_base",
                "demux",
                "-c:v",
                "png",
                "-threads:v",
                "1",
                "-f",
                "null",
                "-",
            ];
            if let Err(e) = runner(&path, &args, PROBE_TIMEOUT) {
                tool.issues
                    .push(format!("Artwork timestamp options failed: {e}"));
            }
        }
        if tool.issues.is_empty() {
            return tool;
        }
        // If no complete build exists, operations that only need some features
        // can still use a working binary (e.g. image import without libx264).
        if first
            .as_ref()
            .is_none_or(|p: &ToolStatus| p.version.is_none() && tool.version.is_some())
        {
            first = Some(tool);
        }
    }
    first.unwrap_or_else(|| ToolStatus {
        name,
        path: None,
        version: None,
        issues: vec![format!("Could not find {name}.")],
    })
}

fn probe(dirs: &[PathBuf], package: Package, runner: &Runner<'_>) -> Report {
    let tools = vec![
        inspect_binary(dirs, Binary::Ffmpeg, runner),
        inspect_binary(dirs, Binary::Ffprobe, runner),
    ];
    let ready = tools.iter().all(|t| t.issues.is_empty());
    let spec = package.spec();
    Report {
        ready,
        tools,
        install_command: spec.map(|(name, args)| format!("{name} {}", args.join(" "))),
        manager_available: spec.is_some_and(|(name, _)| !find(dirs, name).is_empty()),
        manager_help: package.help(),
    }
}

fn install(dirs: &[PathBuf], package: Package, runner: &Runner<'_>) -> Result<Report, String> {
    let before = probe(dirs, package, runner);
    if before.ready {
        return Ok(before);
    }
    let (name, args) = package.spec().ok_or(package.help())?;
    let path = find(dirs, name).into_iter().next().ok_or(package.help())?;
    runner(&path, args, INSTALL_TIMEOUT)?;
    let after = probe(dirs, package, runner);
    if after.ready {
        Ok(after)
    } else {
        let issues: Vec<_> = after
            .tools
            .iter()
            .flat_map(|t| t.issues.iter().map(|s| format!("{}: {s}", t.name)))
            .collect();
        Err(format!(
            "Installation completed, but the media tools still need attention. {}",
            issues.join(" ")
        ))
    }
}

fn refresh() -> Report {
    let mut stored = RESOLVED.lock().unwrap_or_else(|e| e.into_inner());
    let report = probe(&crate::lyrics::tool_dirs(), Package::local(), &run);
    *stored = Some(report.clone());
    report
}

pub(crate) fn command(binary: Binary) -> Result<Command, String> {
    let mut stored = RESOLVED.lock().unwrap_or_else(|e| e.into_inner());
    if stored.as_ref().is_none_or(|r| {
        !r.tools.iter().any(|t| {
            t.name == binary.name()
                && t.version.is_some()
                && t.path.as_ref().is_some_and(|p| p.is_file())
        })
    }) {
        *stored = Some(probe(&crate::lyrics::tool_dirs(), Package::local(), &run));
    }
    let tool = stored
        .as_ref()
        .unwrap()
        .tools
        .iter()
        .find(|t| t.name == binary.name())
        .unwrap();
    let path = tool
        .path
        .as_ref()
        .filter(|_| tool.version.is_some())
        .ok_or_else(|| {
            format!(
                "{} Open FFmpeg media tools in the editor to install or recheck.",
                tool.issues.join(" ")
            )
        })?;
    Ok(Command::new(path))
}

#[tauri::command]
pub(crate) async fn check_media_tools() -> Result<Report, String> {
    tauri::async_runtime::spawn_blocking(refresh)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn install_media_tools() -> Result<Report, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = INSTALL
            .try_lock()
            .map_err(|_| "A media tool installation is already in progress.")?;
        let result = install(&crate::lyrics::tool_dirs(), Package::local(), &run);
        // A failed package operation can still change links or install one tool.
        // The next import or explicit recheck must see its current state.
        *RESOLVED.lock().unwrap_or_else(|e| e.into_inner()) = result.as_ref().ok().cloned();
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, fs};

    #[cfg(unix)]
    #[test]
    fn installed_codec_link_resolves_absolute_and_relative_package_targets() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let links = root.path().join("Links");
        let package = root.path().join("Packages");
        std::fs::create_dir(&links).unwrap();
        std::fs::create_dir(&package).unwrap();
        let ffmpeg = package.join("ffmpeg.exe");
        std::fs::write(&ffmpeg, b"fixture").unwrap();
        let absolute = links.join("ffmpeg.exe");
        let relative = links.join("ffprobe.exe");
        symlink(&ffmpeg, &absolute).unwrap();
        symlink("../Packages/ffmpeg.exe", &relative).unwrap();
        assert_eq!(linked_executable(&absolute).unwrap(), ffmpeg);
        assert_eq!(
            linked_executable(&relative)
                .unwrap()
                .canonicalize()
                .unwrap(),
            ffmpeg.canonicalize().unwrap()
        );
        std::fs::remove_file(&ffmpeg).unwrap();
        assert!(linked_executable(&absolute).is_none());
    }

    fn entries(dir: &Path, names: &[&str]) {
        for name in names {
            fs::write(
                dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
                [],
            )
            .unwrap();
        }
    }
    fn healthy(path: &Path, args: &[&str], _: Duration) -> Result<String, String> {
        match (path.file_stem().unwrap().to_str().unwrap(),args.last().copied()){
            ("ffprobe",Some("json"))=>Ok(r#"{"program_version":{"version":"8.1"}}"#.into()),
            ("ffmpeg",Some("-")) if args.contains(&"-enc_time_base")=>Ok(String::new()),
            ("ffmpeg",Some("-version"))=>Ok("ffmpeg version 8.1 fixture".into()),
            ("ffmpeg",Some("-encoders"))=>Ok("V libx264\nA aac\nV png".into()),
            ("ffmpeg",Some("-decoders"))=>Ok("V png\nV rawvideo\nV libvpx\nV libvpx-vp9".into()),
            ("ffmpeg",Some("-filters"))=>Ok(".. colorspace\n.. scale\n.. format\n.. setpts\n.. setsar\n.. premultiply\n.. unpremultiply".into()),
            ("ffmpeg",Some("-muxers"))=>Ok("E mov\nE mp4".into()),
            _=>panic!("Unexpected program or argument: {path:?} {args:?}"),
        }
    }
    #[test]
    fn installed_package_is_a_noop_and_fixed_install_commands_recheck_new_links() {
        for package in [Package::Brew, Package::Winget] {
            let tmp = tempfile::tempdir().unwrap();
            let dirs = [tmp.path().to_owned()];
            let (manager, arguments) = package.spec().unwrap();
            if matches!(package, Package::Brew) {
                assert_eq!(arguments, ["install", "ffmpeg"]);
            } else {
                assert_eq!(
                    arguments,
                    [
                        "install",
                        "--id",
                        "Gyan.FFmpeg",
                        "--exact",
                        "--source",
                        "winget",
                        "--accept-package-agreements",
                        "--accept-source-agreements",
                        "--disable-interactivity"
                    ]
                );
            }
            entries(tmp.path(), &[manager]);
            let calls = Cell::new(0);
            let runner = |path: &Path, args: &[&str], duration: Duration| {
                if path.file_stem().unwrap() == manager {
                    assert_eq!(args, arguments);
                    assert_eq!(duration, INSTALL_TIMEOUT);
                    calls.set(calls.get() + 1);
                    entries(tmp.path(), &["ffmpeg", "ffprobe"]);
                    Ok(String::new())
                } else {
                    healthy(path, args, duration)
                }
            };
            let missing = probe(&dirs, package, &runner);
            assert!(!missing.ready && missing.manager_available);
            assert_eq!(
                missing.install_command.unwrap(),
                format!("{manager} {}", arguments.join(" "))
            );
            assert!(install(&dirs, package, &runner).unwrap().ready);
            assert!(install(&dirs, package, &runner).unwrap().ready);
            assert_eq!(calls.get(), 1);
        }
    }
    #[test]
    fn missing_manager_failed_install_subsequent_repair_and_duplicate_click() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = [tmp.path().to_owned()];
        assert_eq!(
            install(&dirs, Package::Brew, &healthy).err().unwrap(),
            Package::Brew.help()
        );
        entries(tmp.path(), &["brew"]);
        let failure = |_: &Path, _: &[&str], _: Duration| {
            Err("Network unavailable for package download".into())
        };
        assert!(install(&dirs, Package::Brew, &failure)
            .err()
            .unwrap()
            .contains("Network unavailable"));
        let incomplete = |path: &Path, args: &[&str], duration| {
            if path.file_stem().unwrap() == "brew" {
                entries(tmp.path(), &["ffmpeg"]);
                return Ok(String::new());
            }
            if args.last() == Some(&"-encoders") {
                return Ok("V png\nA aac".into());
            }
            healthy(path, args, duration)
        };
        let err = install(&dirs, Package::Brew, &incomplete).err().unwrap();
        assert!(err.contains("libx264") && err.contains("ffprobe"));
        entries(tmp.path(), &["ffprobe"]);
        assert!(install(&dirs, Package::Brew, &healthy).unwrap().ready);
        let guard = INSTALL.lock().unwrap();
        assert!(tauri::async_runtime::block_on(install_media_tools())
            .err()
            .unwrap()
            .contains("already in progress"));
        drop(guard);
    }
    #[test]
    fn complete_build_after_a_broken_path_entry_is_used_for_import_and_export() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        entries(first.path(), &["ffmpeg", "ffprobe"]);
        entries(second.path(), &["ffmpeg", "ffprobe"]);
        let runner = |p: &Path, args: &[&str], d| {
            if p.parent() == Some(first.path()) {
                if args.last() == Some(&"-encoders") {
                    Ok("V aac\nV png".into())
                } else if p.file_stem().unwrap() == "ffprobe" {
                    Ok("broken probe output".into())
                } else {
                    healthy(p, args, d)
                }
            } else {
                healthy(p, args, d)
            }
        };
        let dirs = [first.path().to_owned(), second.path().to_owned()];
        let report = probe(&dirs, Package::Brew, &runner);
        assert!(report.ready);
        assert!(report
            .tools
            .iter()
            .all(|t| t.path.as_ref().unwrap().parent() == Some(second.path())
                && t.version.as_deref() == Some("8.1")));
    }
    #[test]
    #[ignore = "physical-machine FFmpeg/FFprobe inspection; never installs a package"]
    fn installed_host_capabilities_and_install_noop() {
        let dirs = crate::lyrics::tool_dirs();
        let package = Package::local();
        let read_only = |p: &Path, args: &[&str], d| {
            assert!(
                matches!(
                    p.file_stem().and_then(|n| n.to_str()),
                    Some("ffmpeg" | "ffprobe")
                ),
                "Unexpected package manager invocation"
            );
            run(p, args, d)
        };
        let before = probe(&dirs, package, &read_only);
        assert!(
            before.ready,
            "{}",
            serde_json::to_string_pretty(&before).unwrap()
        );
        let report = install(&dirs, package, &read_only).unwrap();
        assert!(report.ready);
        for binary in [Binary::Ffmpeg, Binary::Ffprobe] {
            let detected = report
                .tools
                .iter()
                .find(|t| t.name == binary.name())
                .unwrap();
            assert_eq!(
                command(binary).unwrap().get_program(),
                detected.path.as_ref().unwrap()
            );
        }
        for t in report.tools {
            println!(
                "{} {} {}",
                t.name,
                t.version.unwrap(),
                t.path.unwrap().display()
            );
        }
    }
}
