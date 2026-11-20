//! Operator-started controller helper. Its owned stdin pipe is also its lifetime.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct Performer {
    process: Mutex<Option<Child>>,
    generation: AtomicU64,
}
#[derive(Deserialize, Serialize)]
pub struct ListenConfig {
    bind: String,
    host: String,
    port: u16,
    cert: String,
    key: String,
}
#[derive(Serialize)]
pub struct Pairing {
    url: String,
    secret: String,
}
#[tauri::command]
pub async fn choose_performer_tls(app: AppHandle, key: bool) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = app
            .dialog()
            .file()
            .set_title(if key {
                "Choose the controller TLS private key"
            } else {
                "Choose the controller TLS certificate chain"
            })
            .add_filter("PEM certificate or key", &["pem", "key", "crt", "cer"]);
        dialog
            .blocking_pick_file()
            .map(|p| {
                p.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
}
fn helper() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = if cfg!(windows) {
        "tss-performer.exe"
    } else {
        "tss-performer"
    };
    Ok(exe.with_file_name(name))
}
#[tauri::command]
pub async fn start_performer(app: AppHandle, config: ListenConfig) -> Result<Pairing, String> {
    tauri::async_runtime::spawn_blocking(move || start(app, config, false))
        .await
        .map_err(|e| e.to_string())?
}
fn start(app: AppHandle, config: ListenConfig, proof: bool) -> Result<Pairing, String> {
    let state = app.state::<Performer>();
    let mut slot = state.process.lock().map_err(|e| e.to_string())?;
    if let Some(c) = slot.as_mut() {
        if c.try_wait().map_err(|e| e.to_string())?.is_none() {
            return Err("The performance listener is already running".into());
        }
    }
    let rev = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let mut cmd = Command::new(helper()?);
    if proof {
        cmd.arg("--verify-loopback");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Start tss-performer beside the editor: {e}"))?;
    let setup = (|| {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or("Missing listener control channel")?;
        serde_json::to_writer(&mut *stdin, &config).map_err(|e| e.to_string())?;
        stdin.write_all(b"\n").map_err(|e| e.to_string())?;
        stdin.flush().map_err(|e| e.to_string())?;
        let mut out = BufReader::new(
            child
                .stdout
                .take()
                .ok_or("Missing listener input channel")?,
        );
        let mut line = String::new();
        out.read_line(&mut line).map_err(|e| e.to_string())?;
        let ready: Value =
            serde_json::from_str(&line).map_err(|e| format!("Controller startup: {e}"))?;
        if ready["kind"] != "ready" {
            return Err(ready["error"]
                .as_str()
                .unwrap_or("Listener failed to start")
                .to_owned());
        }
        let pairing = Pairing {
            url: ready["url"]
                .as_str()
                .ok_or("Missing controller URL")?
                .into(),
            secret: ready["secret"]
                .as_str()
                .ok_or("Missing pairing secret")?
                .into(),
        };
        let events = app.clone();
        std::thread::Builder::new()
            .name("TSS performer input".into())
            .spawn(move || {
                for line in out.lines().map_while(Result::ok) {
                    if events
                        .state::<Performer>()
                        .generation
                        .load(Ordering::SeqCst)
                        != rev
                    {
                        return;
                    }
                    if let Ok(value) = serde_json::from_str::<Value>(&line) {
                        let _ = events.emit("tss-performer-batch", &value);
                    }
                }
                if events
                    .state::<Performer>()
                    .generation
                    .load(Ordering::SeqCst)
                    == rev
                {
                    let _ =
                        events.emit("tss-performer-batch", serde_json::json!({"kind":"stopped"}));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(pairing)
    })();
    match setup {
        Ok(p) => {
            *slot = Some(child);
            Ok(p)
        }
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(e)
        }
    }
}
#[tauri::command]
pub async fn start_performer_verification(app: AppHandle) -> Result<bool, String> {
    if !crate::export_smoke::enabled() || !std::env::args_os().any(|s| s == "--verify-performers") {
        return Ok(false);
    }
    tauri::async_runtime::spawn_blocking(move || {
        start(
            app,
            ListenConfig {
                bind: "127.0.0.1".into(),
                host: "127.0.0.1".into(),
                port: 0,
                cert: String::new(),
                key: String::new(),
            },
            true,
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(true)
}
#[tauri::command]
pub fn performer_verification_step(
    state: State<'_, Performer>,
    step: String,
) -> Result<(), String> {
    if !crate::export_smoke::enabled()
        || !std::env::args_os().any(|s| s == "--verify-performers")
        || !matches!(step.as_str(), "join" | "hold" | "release")
    {
        return Err("Invalid performer verification request".into());
    }
    let mut lock = state.process.lock().map_err(|e| e.to_string())?;
    let input = lock
        .as_mut()
        .and_then(|c| c.stdin.as_mut())
        .ok_or("Listener stopped")?;
    writeln!(input, "{step}")
        .and_then(|_| input.flush())
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn stop_performer(state: State<'_, Performer>) -> Result<(), String> {
    let mut slot = state.process.lock().map_err(|e| e.to_string())?;
    state.generation.fetch_add(1, Ordering::SeqCst);
    if let Some(mut c) = slot.take() {
        drop(c.stdin.take());
        let until = Instant::now() + Duration::from_secs(2);
        while c.try_wait().map_err(|e| e.to_string())?.is_none() {
            if Instant::now() >= until {
                c.kill().map_err(|e| e.to_string())?;
                c.wait().map_err(|e| e.to_string())?;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}
