use clap::{Parser, Subcommand};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
use wait_timeout::ChildExt;
const QUERY: &str =
    "index,uuid,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw";
// Windows Job Objects retain an identity-stable handle while draining.
#[cfg(windows)]
const SCOPE_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    action: Action,
}
#[derive(Subcommand)]
enum Action {
    Run {
        #[arg(long)]
        reservation: PathBuf,
        #[arg(long)]
        timeout: f64,
        #[arg(long)]
        telemetry_csv: Option<PathBuf>,
        #[arg(long, default_value_t = 1.)]
        interval: f64,
        #[arg(long, default_value_t = 5.)]
        smi_timeout: f64,
        #[arg(long, default_value = "nvidia-smi")]
        smi_program: OsString,
        #[arg(long, hide = true)]
        smi_arg: Vec<OsString>,
        #[arg(long)]
        expected_nvidia_uuid: Option<String>,
        #[arg(long)]
        require_idle: bool,
        #[arg(long, default_value_t = 5.)]
        idle_max_utilization: f64,
        #[arg(long)]
        output_log: Option<PathBuf>,
        #[arg(long)]
        progress_timeout: Option<f64>,
        #[arg(last = true, required = true)]
        command: Vec<OsString>,
    },
    Release {
        #[arg(long)]
        reservation: PathBuf,
        #[arg(long)]
        token: String,
        #[arg(long)]
        confirm_stopped: bool,
        #[arg(long)]
        recover_stale: bool,
    },
}
#[derive(Serialize, Deserialize)]
struct Owner {
    token: String,
    wrapper_pid: u32,
    started_unix_ms: u128,
}
#[derive(Serialize)]
struct Outcome {
    result: String,
    pid: Option<u32>,
    exit_code: Option<i32>,
}
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
fn seconds(v: f64) -> Result<Duration> {
    if !v.is_finite() || v <= 0. || v > 86400. {
        return Err("duration must be finite and within (0,86400] seconds".into());
    }
    Ok(Duration::from_secs_f64(v))
}
fn guard_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".guard");
    s.into()
}
fn guard(path: &Path) -> Result<File> {
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(guard_path(path))?;
    f.try_lock_exclusive()
        .map_err(|e| format!("reservation guard unavailable: {e}"))?;
    Ok(f)
}
fn atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    serde_json::to_writer(&mut f, value)?;
    f.write_all(b"\n")?;
    f.sync_all()?;
    drop(f);
    fs::rename(tmp, path)?;
    Ok(())
}
fn remove_reservation(path: &Path) -> Result<()> {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".released-{}", Uuid::new_v4()));
    fs::rename(path, &name)?;
    fs::remove_dir_all(name)?;
    Ok(())
}
fn query_nvidia(
    program: &OsString,
    prefix: &[OsString],
    query: &str,
    timeout: Duration,
) -> Result<Vec<u8>> {
    const MAX_SAMPLE_BYTES: u64 = 1_048_576;
    #[cfg(windows)]
    let deadline = Instant::now() + timeout;
    // A file, unlike a pipe, does not require descendants to close inherited
    // stdout before the completed query can be read. Bound the data accepted.
    let mut output = tempfile::tempfile()?;
    let mut args = vec![program.clone()];
    args.extend_from_slice(prefix);
    args.push(query.into());
    args.push("--format=csv,noheader,nounits".into());
    let args: Vec<String> = args
        .into_iter()
        .map(|a| {
            a.into_string()
                .map_err(|_| "non-Unicode telemetry argument")
        })
        .collect::<std::result::Result<_, _>>()?;
    let scope = scope::Scope::new()?;
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("--bootstrap")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::null());
    scope.prepare(&mut cmd);
    let mut child = cmd.spawn()?;
    if let Err(e) = scope.assign(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    let mut running = Running {
        scope,
        child,
        disarmed: false,
    };
    let mut pipe = running
        .child
        .stdin
        .take()
        .ok_or("telemetry bootstrap has no stdin")?;
    serde_json::to_writer(&mut pipe, &args)?;
    pipe.write_all(b"\n")?;
    drop(pipe);
    #[cfg(windows)]
    let wait = {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("NVIDIA telemetry timed out".into());
        }
        remaining
    };
    #[cfg(not(windows))]
    let wait = timeout;
    let status = match running.child.wait_timeout(wait)? {
        Some(s) => s,
        None => return Err("NVIDIA telemetry timed out".into()),
    };
    if !status.success() {
        return Err(format!("NVIDIA telemetry failed: {status}").into());
    }
    #[cfg(windows)]
    {
        let drain_deadline = (Instant::now() + SCOPE_DRAIN_TIMEOUT).min(deadline);
        while !running.scope.empty(&running.child)? {
            if Instant::now() >= drain_deadline {
                return Err("NVIDIA telemetry left a running descendant".into());
            }
            thread::sleep(
                Duration::from_millis(25)
                    .min(drain_deadline.saturating_duration_since(Instant::now())),
            );
        }
        if Instant::now() >= deadline {
            return Err("NVIDIA telemetry timed out".into());
        }
    }
    #[cfg(not(windows))]
    if !running.scope.empty(&running.child)? {
        return Err("NVIDIA telemetry left a running descendant".into());
    }
    running.disarmed = true;
    if output.metadata()?.len() > MAX_SAMPLE_BYTES {
        return Err("NVIDIA telemetry exceeded maximum sample size".into());
    }
    output.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    output.take(MAX_SAMPLE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SAMPLE_BYTES {
        return Err("NVIDIA telemetry exceeded maximum sample size".into());
    }
    Ok(bytes)
}
fn preflight_sample(
    program: &OsString,
    prefix: &[OsString],
    timeout: Duration,
) -> Result<Vec<Vec<String>>> {
    let bytes = query_nvidia(program, prefix, &format!("--query-gpu={QUERY}"), timeout)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(bytes.as_slice());
    let mut rows = Vec::new();
    for row in reader.records() {
        let row = row?;
        if row.len() != 7 || row[1].trim().is_empty() {
            return Err("malformed GPU sample".into());
        }
        for i in [0, 2, 3, 4, 5, 6] {
            let v: f64 = row[i].trim().parse()?;
            if !v.is_finite() || v < 0. {
                return Err("invalid GPU sample value".into());
            }
        }
        rows.push(row.iter().map(|x| x.trim().to_owned()).collect());
    }
    if rows.is_empty() {
        return Err("no NVIDIA GPU telemetry".into());
    }
    Ok(rows)
}
fn sample_to_csv(
    writer: &mut csv::Writer<File>,
    program: &OsString,
    prefix: &[OsString],
    timeout: Duration,
) -> Result<Vec<Vec<String>>> {
    let now = now_ms().to_string();
    let rows = preflight_sample(program, prefix, timeout)?;
    for row in &rows {
        writer.write_record(std::iter::once(now.as_str()).chain(row.iter().map(String::as_str)))?;
    }
    writer.flush()?;
    Ok(rows)
}
fn verify_idle(
    program: &OsString,
    prefix: &[OsString],
    timeout: Duration,
    uuid: &str,
    max_util: f64,
    rows: &[Vec<String>],
) -> Result<()> {
    let matching: Vec<_> = rows.iter().filter(|r| r[1] == uuid).collect();
    if matching.len() != 1 {
        return Err("expected NVIDIA UUID not uniquely present".into());
    }
    let utilization: f64 = matching[0][2].parse()?;
    if utilization > max_util {
        return Err(format!("preflight GPU utilization {utilization}% exceeds {max_util}%").into());
    }
    let bytes = query_nvidia(
        program,
        prefix,
        "--query-compute-apps=gpu_uuid,pid",
        timeout,
    )?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(bytes.as_slice());
    for record in reader.records() {
        let r = record?;
        if r.len() != 2
            || !r[0].trim().starts_with("GPU-")
            || r[1].trim().parse::<u32>().ok().filter(|v| *v > 0).is_none()
        {
            return Err("unavailable or malformed NVIDIA compute-process query".into());
        }
        if r[0].trim() == uuid {
            return Err("NVIDIA reports an active compute process on expected GPU".into());
        }
    }
    println!("NVIDIA preflight UUID {uuid}, sampled utilization {utilization}%, no visible compute applications");
    Ok(())
}
#[cfg(unix)]
mod scope {
    use super::*;
    use std::os::unix::process::CommandExt;
    pub struct Scope;
    impl Scope {
        pub fn new() -> Result<Self> {
            Ok(Self)
        }
        pub fn prepare(&self, cmd: &mut Command) {
            cmd.process_group(0);
        }
        pub fn assign(&self, _: &Child) -> Result<()> {
            Ok(())
        }
        pub fn empty(&self, child: &Child) -> Result<bool> {
            let rc = unsafe { libc::kill(-(child.id() as i32), 0) };
            if rc == 0 {
                return Ok(false);
            }
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ESRCH) {
                Ok(true)
            } else {
                Err(e.into())
            }
        }
        pub fn terminate(&self, child: &mut Child) {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[cfg(windows)]
mod scope {
    use super::*;
    use std::{
        mem::{size_of, zeroed},
        os::windows::io::AsRawHandle,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
            TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
    };
    pub struct Scope(HANDLE);
    impl Scope {
        pub fn new() -> Result<Self> {
            unsafe {
                let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if h.is_null() {
                    return Err(io::Error::last_os_error().into());
                }
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as _,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    let e = io::Error::last_os_error();
                    CloseHandle(h);
                    return Err(e.into());
                }
                Ok(Self(h))
            }
        }
        pub fn prepare(&self, _: &mut Command) {}
        pub fn assign(&self, child: &Child) -> Result<()> {
            unsafe {
                if AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) == 0 {
                    Err(io::Error::last_os_error().into())
                } else {
                    Ok(())
                }
            }
        }
        pub fn empty(&self, _: &Child) -> Result<bool> {
            unsafe {
                let mut data: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = zeroed();
                if QueryInformationJobObject(
                    self.0,
                    JobObjectBasicAccountingInformation,
                    &mut data as *mut _ as _,
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                ) == 0
                {
                    return Err(io::Error::last_os_error().into());
                }
                Ok(data.ActiveProcesses == 0)
            }
        }
        pub fn terminate(&self, child: &mut Child) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
#[cfg(not(any(unix, windows)))]
compile_error!("unsupported platform");
struct Running {
    scope: scope::Scope,
    child: Child,
    disarmed: bool,
}
impl Drop for Running {
    fn drop(&mut self) {
        if !self.disarmed {
            self.scope.terminate(&mut self.child);
        }
    }
}
fn bootstrap() -> Result<i32> {
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let args: Vec<String> = serde_json::from_str(&line)?;
    if args.is_empty() {
        return Err("empty bootstrap command".into());
    }
    let status = Command::new(&args[0]).args(&args[1..]).status()?;
    Ok(status.code().unwrap_or(1))
}
struct RunOptions {
    path: PathBuf,
    timeout: Duration,
    csv_path: Option<PathBuf>,
    interval: Duration,
    smi_timeout: Duration,
    smi: OsString,
    smi_args: Vec<OsString>,
    expected_nvidia_uuid: Option<String>,
    require_idle: bool,
    idle_max_utilization: f64,
    output_log: Option<PathBuf>,
    progress_timeout: Option<Duration>,
    command: Vec<OsString>,
}
fn run(options: RunOptions) -> Result<i32> {
    let RunOptions {
        path,
        timeout,
        csv_path,
        interval,
        smi_timeout,
        smi,
        smi_args,
        expected_nvidia_uuid,
        require_idle,
        idle_max_utilization,
        output_log,
        progress_timeout,
        command,
    } = options;
    let command: Vec<String> = command
        .into_iter()
        .map(|x| x.into_string().map_err(|_| "non-Unicode command argument"))
        .collect::<std::result::Result<_, _>>()?;
    if command.is_empty() {
        return Err("empty command".into());
    }
    if require_idle && (expected_nvidia_uuid.is_none() || csv_path.is_none()) {
        return Err("require-idle needs expected-nvidia-uuid and telemetry-csv".into());
    }
    if expected_nvidia_uuid.is_some() && !require_idle {
        return Err("expected-nvidia-uuid currently requires require-idle".into());
    }
    if require_idle
        && (!idle_max_utilization.is_finite() || !(0.0..=100.0).contains(&idle_max_utilization))
    {
        return Err("invalid idle-max-utilization".into());
    }
    if progress_timeout.is_some() && output_log.is_none() {
        return Err("progress-timeout requires output-log".into());
    }
    let _guard = guard(&path)?;
    fs::create_dir(&path).map_err(|e| format!("reservation unavailable: {e}"))?;
    let owner = Owner {
        token: Uuid::new_v4().to_string(),
        wrapper_pid: std::process::id(),
        started_unix_ms: now_ms(),
    };
    atomic_json(&path.join("owner.json"), &owner)?;
    println!(
        "GPU reservation: {}; release token: {}",
        path.display(),
        owner.token
    );
    let deadline = Instant::now() + timeout;
    let mut outcome = Outcome {
        result: "setup_failed".into(),
        pid: None,
        exit_code: None,
    };
    let mut clean = false;
    let result = (|| -> Result<i32> {
        let mut writer = if let Some(file) = csv_path {
            let file = OpenOptions::new().write(true).create_new(true).open(file)?;
            let mut w = csv::Writer::from_writer(file);
            w.write_record([
                "sample_unix_ms",
                "index",
                "uuid",
                "utilization_gpu_percent",
                "memory_used_mib",
                "memory_total_mib",
                "temperature_c",
                "power_w",
            ])?;
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                outcome.result = "timeout".into();
                return Ok(124);
            }
            let rows = match sample_to_csv(&mut w, &smi, &smi_args, left.min(smi_timeout)) {
                Ok(rows) => rows,
                Err(e) => {
                    if Instant::now() >= deadline {
                        outcome.result = "timeout".into();
                        return Ok(124);
                    }
                    return Err(e);
                }
            };
            if require_idle {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    outcome.result = "timeout".into();
                    return Ok(124);
                }
                if let Err(e) = verify_idle(
                    &smi,
                    &smi_args,
                    left.min(smi_timeout),
                    expected_nvidia_uuid.as_deref().unwrap(),
                    idle_max_utilization,
                    &rows,
                ) {
                    if Instant::now() >= deadline {
                        outcome.result = "timeout".into();
                        return Ok(124);
                    }
                    return Err(e);
                }
            }
            Some(w)
        } else {
            None
        };
        if Instant::now() >= deadline {
            outcome.result = "timeout".into();
            return Ok(124);
        }
        let interrupted = Arc::new(AtomicBool::new(false));
        let sig = interrupted.clone();
        ctrlc::set_handler(move || sig.store(true, Ordering::Release))?;
        let scope = scope::Scope::new()?;
        let mut cmd = Command::new(std::env::current_exe()?);
        cmd.arg("--bootstrap").stdin(Stdio::piped());
        if let Some(path) = &output_log {
            let log = OpenOptions::new().write(true).create_new(true).open(path)?;
            cmd.stdout(Stdio::from(log.try_clone()?))
                .stderr(Stdio::from(log));
        }
        scope.prepare(&mut cmd);
        let mut child = cmd.spawn()?;
        if let Err(e) = scope.assign(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        let mut running = Running {
            scope,
            child,
            disarmed: false,
        };
        outcome.pid = Some(running.child.id());
        atomic_json(
            &path.join("child.json"),
            &serde_json::json!({"pid": running.child.id()}),
        )?;
        if Instant::now() >= deadline {
            outcome.result = "timeout".into();
            return Ok(124);
        }
        let mut pipe = running.child.stdin.take().ok_or("bootstrap has no stdin")?;
        serde_json::to_writer(&mut pipe, &command)?;
        pipe.write_all(b"\n")?;
        drop(pipe);
        println!("GPU job process: {}", running.child.id());
        let mut last_progress = Instant::now();
        let mut progress_state: Option<(u64, Option<SystemTime>)> = None;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let sampler = writer.take().map(|mut w| {
            let stop = stop.clone();
            thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    let until = Instant::now() + interval;
                    while !stop.load(Ordering::Acquire) && Instant::now() < until {
                        thread::sleep(
                            Duration::from_millis(20)
                                .min(until.saturating_duration_since(Instant::now())),
                        );
                    }
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    if let Err(e) = sample_to_csv(&mut w, &smi, &smi_args, smi_timeout) {
                        let _ = tx.send(e.to_string());
                        break;
                    }
                }
            })
        });
        #[cfg(windows)]
        let mut drain_deadline: Option<Instant> = None;
        let mut code = loop {
            if Instant::now() >= deadline {
                outcome.result = "timeout".into();
                break 124;
            }
            if interrupted.load(Ordering::Acquire) {
                outcome.result = "interrupted".into();
                break 130;
            }
            if let (Some(limit), Some(path)) = (progress_timeout, &output_log) {
                let md = fs::metadata(path)?;
                let state = (md.len(), md.modified().ok());
                if progress_state != Some(state) {
                    progress_state = Some(state);
                    last_progress = Instant::now();
                }
                if last_progress.elapsed() >= limit {
                    outcome.result = "progress_timeout".into();
                    break 124;
                }
            }
            if let Ok(e) = rx.try_recv() {
                eprintln!("telemetry failed: {e}");
                outcome.result = "telemetry_failed".into();
                break 2;
            }
            if let Some(s) = running.child.try_wait()? {
                outcome.result = "exited".into();
                outcome.exit_code = s.code();
                if s.success() && !running.scope.empty(&running.child)? {
                    #[cfg(windows)]
                    {
                        let until = *drain_deadline.get_or_insert_with(|| {
                            (Instant::now() + SCOPE_DRAIN_TIMEOUT).min(deadline)
                        });
                        if Instant::now() < until {
                            thread::sleep(
                                Duration::from_millis(25)
                                    .min(until.saturating_duration_since(Instant::now())),
                            );
                            continue;
                        }
                    }
                    outcome.result = "descendants_remain".into();
                    break 2;
                }
                break s.code().unwrap_or(1);
            }
            thread::sleep(Duration::from_millis(25));
        };
        if code != 0 {
            running.scope.terminate(&mut running.child);
            running.disarmed = true;
        }
        stop.store(true, Ordering::Release);
        if let Some(t) = sampler {
            if t.join().is_err() {
                outcome.result = "telemetry_failed".into();
                code = 2;
            }
        }
        if let Ok(e) = rx.try_recv() {
            eprintln!("telemetry failed: {e}");
            outcome.result = "telemetry_failed".into();
            code = 2;
        }
        if Instant::now() >= deadline {
            outcome.result = "timeout".into();
            code = 124;
        }
        clean = code == 0 && outcome.result == "exited";
        running.disarmed = true;
        Ok(code)
    })();
    if let Err(ref e) = result {
        eprintln!("job failed: {e}");
    }
    atomic_json(&path.join("outcome.json"), &outcome)?;
    if clean {
        remove_reservation(&path)?;
        println!("GPU reservation released after clean completion");
    } else {
        eprintln!("Reservation retained; verify job tree and device before release");
    }
    result
}
fn main_impl() -> Result<i32> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--bootstrap")) {
        return bootstrap();
    }
    match Cli::parse().action {
        Action::Run {
            reservation,
            timeout,
            telemetry_csv,
            interval,
            smi_timeout,
            smi_program,
            smi_arg,
            expected_nvidia_uuid,
            require_idle,
            idle_max_utilization,
            output_log,
            progress_timeout,
            command,
        } => {
            let (interval, smi_timeout) = if telemetry_csv.is_some() {
                (seconds(interval)?, seconds(smi_timeout)?)
            } else {
                (Duration::from_secs(1), Duration::from_secs(5))
            };
            run(RunOptions {
                path: reservation,
                timeout: seconds(timeout)?,
                csv_path: telemetry_csv,
                interval,
                smi_timeout,
                smi: smi_program,
                smi_args: smi_arg,
                expected_nvidia_uuid,
                require_idle,
                idle_max_utilization,
                output_log,
                progress_timeout: progress_timeout.map(seconds).transpose()?,
                command,
            })
        }
        Action::Release {
            reservation,
            token,
            confirm_stopped,
            recover_stale,
        } => {
            let _guard = guard(&reservation)?;
            let owner: Owner =
                serde_json::from_reader(File::open(reservation.join("owner.json"))?)?;
            if owner.token != token {
                return Err("reservation token does not match".into());
            }
            if !confirm_stopped {
                return Err("confirm-stopped required after independent verification".into());
            }
            if !reservation.join("outcome.json").is_file() && !recover_stale {
                return Err("outcome missing; recover-stale required after verification".into());
            }
            remove_reservation(&reservation)?;
            Ok(0)
        }
    }
}
fn main() {
    let code = match main_impl() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("GPU job refused or failed: {e}");
            2
        }
    };
    std::process::exit(code);
}
