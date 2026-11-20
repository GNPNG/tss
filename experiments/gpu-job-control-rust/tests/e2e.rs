use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
const BIN: &str = env!("CARGO_BIN_EXE_tss-gpu-job");
const FIX: &str = env!("CARGO_BIN_EXE_fixture");
fn run(reservation: &Path, args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(reservation)
        .args(["--timeout", "5"])
        .args(args)
        .output()
        .unwrap()
}
fn wait_exists(path: &Path) {
    let end = Instant::now() + Duration::from_secs(5);
    while !path.exists() && Instant::now() < end {
        thread::sleep(Duration::from_millis(15));
    }
    assert!(path.exists(), "missing {}", path.display());
}
#[test]
fn clean_and_failed_recovery() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    assert!(run(&lock, &["--", FIX, "sleep", "50"]).status.success());
    assert!(!lock.exists());
    assert_eq!(run(&lock, &["--", FIX, "fail"]).status.code(), Some(7));
    assert!(lock.exists());
    let v: serde_json::Value =
        serde_json::from_slice(&fs::read(lock.join("owner.json")).unwrap()).unwrap();
    let t = v["token"].as_str().unwrap();
    assert!(!Command::new(BIN)
        .args(["release", "--reservation"])
        .arg(&lock)
        .args(["--token", "bad", "--confirm-stopped"])
        .status()
        .unwrap()
        .success());
    assert!(!Command::new(BIN)
        .args(["release", "--reservation"])
        .arg(&lock)
        .args(["--token", t])
        .status()
        .unwrap()
        .success());
    assert!(Command::new(BIN)
        .args(["release", "--reservation"])
        .arg(&lock)
        .args(["--token", t, "--confirm-stopped"])
        .status()
        .unwrap()
        .success());
    assert!(!lock.exists());
}
#[test]
fn competition_and_active_release_refused() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let mut first = Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(&lock)
        .args(["--timeout", "5", "--", FIX, "sleep", "1500"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait_exists(&lock.join("child.json"));
    let v: serde_json::Value =
        serde_json::from_slice(&fs::read(lock.join("owner.json")).unwrap()).unwrap();
    let t = v["token"].as_str().unwrap();
    assert_eq!(
        run(&lock, &["--", FIX, "sleep", "20"]).status.code(),
        Some(2)
    );
    assert!(!Command::new(BIN)
        .args(["release", "--reservation"])
        .arg(&lock)
        .args(["--token", t, "--confirm-stopped", "--recover-stale"])
        .status()
        .unwrap()
        .success());
    assert!(first.wait().unwrap().success());
    assert!(!lock.exists());
    let mut second = Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(&lock)
        .args(["--timeout", "5", "--", FIX, "sleep", "500"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait_exists(&lock.join("child.json"));
    assert!(!Command::new(BIN)
        .args(["release", "--reservation"])
        .arg(&lock)
        .args(["--token", t, "--confirm-stopped", "--recover-stale"])
        .status()
        .unwrap()
        .success());
    assert!(second.wait().unwrap().success());
}
#[test]
fn timeout_and_descendant_retention() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let o = Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(&lock)
        .args(["--timeout", "0.2", "--", FIX, "sleep", "3000"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(124));
    assert!(lock.exists());
    let lock = d.path().join("r2");
    let started = d.path().join("start");
    let survived = d.path().join("survived");
    let o = run(
        &lock,
        &[
            "--",
            FIX,
            "spawn-child",
            started.to_str().unwrap(),
            "2000",
            survived.to_str().unwrap(),
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(started.exists());
    assert!(lock.exists());
    thread::sleep(Duration::from_millis(2300));
    assert!(!survived.exists());
}
#[test]
#[cfg(windows)]
fn short_lived_descendant_is_drained_before_release() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("drain");
    let started = d.path().join("started");
    let finished = d.path().join("finished");
    let o = run(
        &lock,
        &[
            "--",
            FIX,
            "spawn-child",
            started.to_str().unwrap(),
            "400",
            finished.to_str().unwrap(),
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(started.exists());
    assert!(finished.exists());
    assert!(!lock.exists());
}

#[test]
#[cfg(windows)]
fn overall_deadline_terminates_descendant_while_draining() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("deadline-drain");
    let started = d.path().join("started");
    let survived = d.path().join("survived");
    let o = Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(&lock)
        .args(["--timeout", "1.0", "--", FIX, "spawn-child"])
        .arg(&started)
        .arg("1300")
        .arg(&survived)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(124), "{:?}", o);
    assert!(started.exists());
    assert!(lock.exists());
    thread::sleep(Duration::from_millis(600));
    assert!(!survived.exists());
}

#[test]
#[cfg(windows)]
fn telemetry_short_lived_descendant_drains_before_sample_acceptance() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("telemetry-drain");
    let csv = d.path().join("telemetry.csv");
    let o = run(
        &lock,
        &[
            "--telemetry-csv",
            csv.to_str().unwrap(),
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi-brief-descendant",
            "--",
            FIX,
            "sleep",
            "50",
        ],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!lock.exists());
    assert!(fs::read_to_string(csv).unwrap().contains("GPU-fake"));
}

#[test]
#[cfg(windows)]
fn telemetry_descendant_cannot_outlive_query_deadline() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("telemetry-deadline");
    let csv = d.path().join("telemetry.csv");
    let marker = d.path().join("should-not-start");
    let telemetry_parent = d.path().join("telemetry-parent");
    let started = Instant::now();
    let o = run(
        &lock,
        &[
            "--telemetry-csv",
            csv.to_str().unwrap(),
            "--smi-timeout",
            "0.8",
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi-inherited-stdout-marker",
            "--smi-arg",
            telemetry_parent.to_str().unwrap(),
            "--",
            FIX,
            "marker-sleep",
            marker.to_str().unwrap(),
            "10",
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{:?}", o);
    assert!(telemetry_parent.exists());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("NVIDIA telemetry left a running descendant")
    );
    assert!(!marker.exists());
    assert!(lock.exists());
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
#[cfg(windows)]
fn telemetry_launch_cannot_succeed_after_query_deadline() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("telemetry-expired");
    let csv = d.path().join("telemetry.csv");
    let marker = d.path().join("should-not-start");
    let o = run(
        &lock,
        &[
            "--telemetry-csv",
            csv.to_str().unwrap(),
            "--smi-timeout",
            "0.000001",
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi",
            "--",
            FIX,
            "marker-sleep",
            marker.to_str().unwrap(),
            "10",
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{:?}", o);
    assert!(String::from_utf8_lossy(&o.stderr).contains("NVIDIA telemetry timed out"));
    assert!(!marker.exists());
    assert!(lock.exists());
}

#[test]
fn telemetry_missing_and_preflight_timeout_do_not_launch() {
    let d = TempDir::new().unwrap();
    let marker = d.path().join("marker");
    let lock = d.path().join("r");
    let log = d.path().join("log.csv");
    let o = run(
        &lock,
        &[
            "--telemetry-csv",
            log.to_str().unwrap(),
            "--smi-program",
            "missing-smi-for-test",
            "--",
            FIX,
            "marker-sleep",
            marker.to_str().unwrap(),
            "20",
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(!marker.exists());
    assert!(lock.exists());
    let lock = d.path().join("r2");
    let o = Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(&lock)
        .args(["--timeout", "0.2", "--telemetry-csv"])
        .arg(d.path().join("slow.csv"))
        .args([
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi-slow",
            "--",
            FIX,
            "marker-sleep",
        ])
        .arg(&marker)
        .arg("20")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(124));
    assert!(!marker.exists());
}
#[test]
fn full_utilization_is_not_failure() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let log = d.path().join("metrics.csv");
    let o = run(
        &lock,
        &[
            "--telemetry-csv",
            log.to_str().unwrap(),
            "--interval",
            "0.05",
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi",
            "--",
            FIX,
            "sleep",
            "400",
        ],
    );
    assert!(o.status.success(), "{:?}", o);
    assert!(!lock.exists());
    assert!(fs::read_to_string(log).unwrap().contains("GPU-fake,100"));
}
#[test]
fn later_telemetry_failure_stops_job() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let log = d.path().join("metrics.csv");
    let count = d.path().join("count");
    let survived = d.path().join("survived");
    let started = d.path().join("started");
    let o = run(
        &lock,
        &[
            "--telemetry-csv",
            log.to_str().unwrap(),
            "--interval",
            "0.1",
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi-fail-after",
            "--smi-arg",
            count.to_str().unwrap(),
            "--",
            FIX,
            "marker-sleep",
            started.to_str().unwrap(),
            "1000",
            survived.to_str().unwrap(),
        ],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(lock.exists());
    assert!(started.exists());
    thread::sleep(Duration::from_millis(1200));
    assert!(!survived.exists());
}
#[test]
fn slow_telemetry_does_not_extend_child_deadline() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let log = d.path().join("metrics.csv");
    let count = d.path().join("count");
    let survived = d.path().join("survived");
    let started = d.path().join("started");
    let o = Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(&lock)
        .args(["--timeout", "0.7", "--telemetry-csv"])
        .arg(log)
        .args([
            "--interval",
            "0.05",
            "--smi-timeout",
            "3",
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi-slow-after",
            "--smi-arg",
        ])
        .arg(count)
        .args(["--", FIX, "marker-sleep"])
        .arg(started)
        .arg("1200")
        .arg(&survived)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(124));
    assert!(!survived.exists());
    assert!(lock.exists());
}
#[test]
fn telemetry_descendant_inheriting_stdout_does_not_hold_pipe() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let log = d.path().join("metrics.csv");
    let start = Instant::now();
    let o = run(
        &lock,
        &[
            "--telemetry-csv",
            log.to_str().unwrap(),
            "--interval",
            "10",
            "--smi-program",
            FIX,
            "--smi-arg",
            "smi-inherited-stdout",
            "--",
            FIX,
            "sleep",
            "10",
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{:?}", o);
    assert!(start.elapsed() < Duration::from_millis(2500));
    assert!(lock.exists());
}
#[test]
fn unused_sampler_options_do_not_block_nontelemetry_job() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let o = run(
        &lock,
        &[
            "--interval",
            "0",
            "--smi-timeout",
            "0",
            "--",
            FIX,
            "sleep",
            "10",
        ],
    );
    assert!(o.status.success(), "{:?}", o);
    assert!(!lock.exists());
}
#[cfg(windows)]
#[test]
fn wrapper_death_terminates_started_child() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let start = d.path().join("start");
    let survived = d.path().join("survived");
    let mut p = Command::new(BIN)
        .args(["run", "--reservation"])
        .arg(&lock)
        .args(["--timeout", "8", "--", FIX, "marker-sleep"])
        .arg(&start)
        .arg("2000")
        .arg(&survived)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait_exists(&start);
    p.kill().unwrap();
    p.wait().unwrap();
    thread::sleep(Duration::from_millis(2200));
    assert!(!survived.exists());
    assert!(lock.exists());
}
#[test]
fn preflight_requires_matching_idle_device_before_child_start() {
    let d = TempDir::new().unwrap();
    let marker = d.path().join("marker");
    for (name, uuid, expected) in [
        ("smi-preflight", "GPU-fake", 0),
        ("smi-preflight", "GPU-wrong", 2),
        ("smi-active", "GPU-fake", 2),
    ] {
        let lock = d.path().join(format!("{}-{}", name, uuid));
        let log = d.path().join(format!("{}-{}.csv", name, uuid));
        let o = run(
            &lock,
            &[
                "--telemetry-csv",
                log.to_str().unwrap(),
                "--smi-program",
                FIX,
                "--smi-arg",
                name,
                "--require-idle",
                "--expected-nvidia-uuid",
                uuid,
                "--",
                FIX,
                "marker-sleep",
                marker.to_str().unwrap(),
                "10",
            ],
        );
        assert_eq!(o.status.code(), Some(expected), "{:?}", o);
        if expected == 0 {
            assert!(marker.exists());
            fs::remove_file(&marker).unwrap();
            assert!(!lock.exists());
        } else {
            assert!(!marker.exists());
            assert!(lock.exists());
        }
    }
}
#[test]
fn progress_watchdog_is_opt_in_and_retains_reservation() {
    let d = TempDir::new().unwrap();
    let lock = d.path().join("r");
    let log = d.path().join("out.log");
    let o = run(
        &lock,
        &[
            "--output-log",
            log.to_str().unwrap(),
            "--progress-timeout",
            "0.2",
            "--",
            FIX,
            "sleep",
            "1000",
        ],
    );
    assert_eq!(o.status.code(), Some(124), "{:?}", o);
    assert!(lock.exists());
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(lock.join("outcome.json")).unwrap()).unwrap();
    assert_eq!(result["result"], "progress_timeout");
    let lock = d.path().join("r2");
    let log = d.path().join("out2.log");
    assert!(run(
        &lock,
        &[
            "--output-log",
            log.to_str().unwrap(),
            "--",
            FIX,
            "sleep",
            "300"
        ]
    )
    .status
    .success());
}
