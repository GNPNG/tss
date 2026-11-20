# GPU experiment reservation and telemetry

`gpu_job.py` is a cooperative per-host reservation for experimental GPU jobs. Every experiment on that host must use the **same reservation path and the same current runner version**. A persistent sibling `.guard` file is held under an OS advisory lock for the full job and during release; do not delete or replace it. A second cooperating runner refuses to start while the reservation or guard is held. Older copies of `gpu_job.py` and other watchdog scripts do not honor this guard and must not be used against the same GPU during a reservation; the OS cannot force a noncooperating process to use this protocol. This is not a system-wide GPU scheduler and does not prevent a GPU, driver or operating-system hang. The investigated project and Windows workspace each had one canonical `gpu_job.py` on the latest search, but scripts elsewhere remain possible.

The runner starts a bootstrap process inside a POSIX process group or Windows Job object before the requested command is allowed to start. The Windows Job object uses `KILL_ON_JOB_CLOSE`, so the OS terminates its processes if the wrapper dies; the reservation directory remains. POSIX has no equivalent crash cleanup in this runner, so a child may survive a wrapper crash. On successful exit, it checks that the process group or Job object has no remaining processes, closes telemetry, and releases the reservation. Commands should be noninteractive and must not deliberately escape the group or arrange GPU work through an independent service. An empty process group is not proof that the GPU or driver has completed all work.

A failed command, timeout, interrupted run, missing telemetry, unexpected descendants, or wrapper crash leaves the reservation in place. The wrapper attempts to terminate its process group or Job object on timeout, interruption, or telemetry failure, but termination cannot guarantee that the GPU or driver recovered. Review the child process tree and GPU state before releasing the reservation. A process crash can happen before `owner.json` is created; the directory still blocks new work.

## Run

Use an existing, trusted Python 3 environment. The reservation's parent directory must already exist. Supply a finite positive timeout. The executable and arguments after `--` are passed as an argument array without a shell; on Windows invoke an executable directly rather than depending on shell syntax or `.bat` file parsing.

```sh
python gpu_job.py run --reservation /path/to/gpu-reservation --timeout 120 -- /path/to/program arg1 arg2
```

For NVIDIA telemetry, add a new CSV path; the runner refuses to overwrite it. The first query must succeed before the child starts. During execution, a failed or timed-out query stops the job and retains the reservation.

```sh
python gpu_job.py run --reservation /path/to/gpu-reservation --timeout 120 \
  --telemetry-csv /path/to/new-job-telemetry.csv --interval 1 \
  -- /path/to/program arg1 arg2
```

The Windows equivalent, from `%USERPROFILE%\work\tss-workspace\gpu-job-control`, is:

```cmd
..\taichi-cuda-eval\mise\python\3.11.12\python.exe gpu_job.py run --reservation "%USERPROFILE%\work\tss-workspace\gpu-job-control\reservation" --timeout 120 --telemetry-csv "%USERPROFILE%\work\tss-workspace\gpu-job-control\job1.csv" --interval 1 -- C:\path\to\program.exe arg1 arg2
```

The telemetry command queries all visible NVIDIA GPUs via `nvidia-smi` for index, UUID, GPU utilization, used and total memory, temperature, and power draw. A query producing no GPUs, a nonzero exit, malformed output, or unavailable/non-numeric requested values is treated as failure. `--smi-program` selects the executable; `--smi-timeout` bounds each query. Sampling is periodic rather than continuous and can miss transient peaks; `nvidia-smi` itself has overhead. Utilization of 100% is not by itself an error and does not stop a job. Telemetry is not a driver-health check or a guarantee of safe temperature, power, or workload duration. In-job sampling runs separately from deadline checks; the timeout bounds the attempt to stop the job, not GPU or driver recovery, and sampler shutdown or OS calls can return later. The sampler checks failures when a query returns or times out, so it cannot detect every fault immediately.

## Recover a retained reservation

First independently verify that the wrapper, child processes and GPU activity have stopped and review the reason for failure. Read the printed token or the `token` field of `<reservation>/owner.json`, then run:

```sh
python gpu_job.py release --reservation /path/to/gpu-reservation --token TOKEN --confirm-stopped
```

If there is no `outcome.json`, add `--recover-stale` only after checking the wrapper and job tree. A compatible active runner holds the guard, so release is denied; an older or noncooperating process may not. When `owner.json` was never created or is damaged, the command fails closed and manual removal of the reservation directory requires the same independent checks. The release token prevents accidental release of a different reservation; it is not an authentication credential.

## Verification

`python -m unittest -v test_gpu_job` runs CPU-only tests, including competing acquisitions and releases, stale tokens, clean release, surviving child detection, timeout retention, slow and missing telemetry, and acceptance of 100% utilization. The Windows crash test waits until the actual child reports startup before terminating the wrapper. These tests do not dispatch GPU workloads or prove behavior for deliberately escaping processes, hardware faults or incompatible runner versions. Actual NVIDIA telemetry, GPU behavior and any particular process tree have to be checked on the host where they run.
