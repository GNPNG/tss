use std::{
    env, fs,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
fn main() {
    let a: Vec<String> = env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("sleep") => thread::sleep(Duration::from_millis(a[2].parse().unwrap())),
        Some("fail") => std::process::exit(7),
        Some("marker-sleep") => {
            fs::write(&a[2], "started").unwrap();
            thread::sleep(Duration::from_millis(a[3].parse().unwrap()));
            if let Some(p) = a.get(4) {
                fs::write(p, "survived").unwrap();
            }
        }
        // This fixture intentionally exits without waiting to exercise cleanup of
        // a still-live descendant; the enclosing test supervisor terminates it.
        #[allow(clippy::zombie_processes)]
        Some("spawn-child") => {
            Command::new(env::current_exe().unwrap())
                .args(["marker-sleep", &a[2], &a[3], &a[4]])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            thread::sleep(Duration::from_millis(250));
        }
        Some("smi") => println!("0, GPU-fake, 100, 100, 4000, 60, 20"),
        Some("smi-preflight") | Some("smi-active") => {
            if a.iter().any(|s| s.starts_with("--query-compute-apps=")) {
                if a[1] == "smi-active" {
                    println!("GPU-fake, 1234");
                }
            } else {
                println!("0, GPU-fake, 1, 100, 4000, 60, 20");
            }
        }
        Some("smi-fail") => std::process::exit(3),
        Some("smi-slow") => {
            thread::sleep(Duration::from_millis(1800));
            println!("0, GPU-fake, 1, 1, 4000, 50, 20");
        }
        // The child intentionally inherits the telemetry output handle. It
        // exits on its own after the bounded test period.
        #[allow(clippy::zombie_processes)]
        Some("smi-inherited-stdout") | Some("smi-inherited-stdout-marker") => {
            Command::new(env::current_exe().unwrap())
                .args(["sleep", "3000"])
                .spawn()
                .unwrap();
            if a[1] == "smi-inherited-stdout-marker" {
                fs::write(&a[2], "telemetry-parent-finished").unwrap();
            }
            println!("0, GPU-fake, 1, 1, 4000, 50, 20");
        }
        #[allow(clippy::zombie_processes)]
        Some("smi-brief-descendant") => {
            Command::new(env::current_exe().unwrap())
                .args(["sleep", "300"])
                .spawn()
                .unwrap();
            println!("0, GPU-fake, 1, 1, 4000, 50, 20");
        }
        Some("smi-fail-after") | Some("smi-slow-after") => {
            let count = fs::read_to_string(&a[2])
                .unwrap_or_default()
                .parse::<u32>()
                .unwrap_or(0);
            fs::write(&a[2], (count + 1).to_string()).unwrap();
            if count > 0 && a[1] == "smi-fail-after" {
                std::process::exit(3);
            }
            if count > 0 && a[1] == "smi-slow-after" {
                thread::sleep(Duration::from_millis(1500));
            }
            println!("0, GPU-fake, 99, 100, 4000, 60, 20");
        }
        _ => std::process::exit(2),
    }
}
