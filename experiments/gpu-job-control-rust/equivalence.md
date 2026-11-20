# Existing runner functionality equivalence inventory

This is a comparison of `../gpu-job-control/gpu_job.py` and this Rust **candidate**, not a declaration of complete equivalence. Yes means the described behavior is implemented and covered by the cited CPU test or direct source inspection, not that a GPU/driver failure is prevented. The program has not been run against an actual GPU. Tests are in `tests/e2e.rs`; Windows-specific coverage ran on the Windows host. Unknown includes mixed-version interoperability and unsupported process behavior.

| Feature / property / behaviour | Legacy implementation | Proposed replacement | Preserved? | Notes / open questions |
|---|---|---|---|---|
| Supported host targets | Python 3 on POSIX and Windows | Rust binary on Unix/Windows | Partial | CPU tests macOS and Windows; Linux and other Unix not tested. |
| Executable invocation | `python gpu_job.py` | `tss-gpu-job` | No | Deliberately different executable and deployment. |
| `run` action | Creates job reservation | Same | Yes | `clean_and_failed_recovery`. |
| `release` action | Token-protected recovery | Same | Yes | `clean_and_failed_recovery`. |
| `--reservation` | Required path converted to absolute | Required path retained as supplied | Partial | Same working-directory relative path works; cross-cwd relative path equivalence not established. Use absolute paths. |
| Persistent `.guard` | `flock` or `msvcrt.locking`, retained | `fs2` OS advisory exclusive file lock, retained | Partial | Cooperating Rust copies tested; cross-protocol Python/Rust lock interoperability **unknown**. Never mix protocols. |
| Guard acquisition nonblocking | Refuse on contention | Same | Yes | `competition_and_active_release_refused`. |
| Guard lifetime | Full run and release transaction | Same | Yes | `competition_and_active_release_refused`. |
| Atomic reservation directory | `mkdir`, fails on existing path | `create_dir`, fails on existing path | Yes | `competition_and_active_release_refused`. |
| Existing directory / stale owner | Refuse new job | Same | Yes | `clean_and_failed_recovery`, `timeout_and_descendant_retention`. |
| Crash before owner metadata | Directory remains; manual recovery | Same | Unknown | Source ordering matches; crash at exact point not tested. |
| Token generation | UUID4 hex | UUID4 hyphenated | Partial | Still random; textual representation differs. |
| `owner.json` token and wrapper PID | Saved | Saved | Yes | `clean_and_failed_recovery`, source. |
| Owner start time | ISO8601 UTC | Unix epoch milliseconds | No | Different field name and representation. |
| Metadata writing | Exclusive temporary file, fsync, rename | Same sequence | Yes | Source inspection; failure injection not tested. |
| `child.json` PID | Bootstrap PID saved | Same | Yes | `competition_and_active_release_refused`. |
| `outcome.json` result/PID/exit code | Saved, with token | Saved, without token | Partial | Outcome schema differs; `timeout_and_descendant_retention`. |
| Outcome when parent dies | Absent, reservation remains | Same | Yes | Windows `wrapper_death_terminates_started_child`. |
| Clean exit removal | Confirm no descendants, then remove | Same | Yes | `clean_and_failed_recovery`. |
| Failed command exit propagation | Nonzero status returned, reservation remains | Same for numeric exit codes | Partial | Signal-terminated bootstrap maps to 1; Python maps negative return code to 128+signal. |
| Whole-job deadline | Finite positive seconds | Finite positive seconds, maximum 86400 | Partial | Additional limit; `timeout_and_descendant_retention`. |
| Deadline includes preflight | Yes | Yes | Yes | `telemetry_missing_and_preflight_timeout_do_not_launch`. |
| Deadline checked during run | Separate from telemetry thread | Same | Yes | `slow_telemetry_does_not_extend_child_deadline`. |
| Deadline checked after exit | Yes | Yes | Yes | Source inspection; exact boundary race not independently stressed. |
| `--timeout` outcome | 124, retains reservation, tries cleanup | Same | Yes | `timeout_and_descendant_retention`. |
| `--interval` | Finite positive seconds when telemetry enabled, default 1 | Same plus 86400-second maximum | Partial | `unused_sampler_options_do_not_block_nontelemetry_job` checks disabled telemetry. |
| `--smi-timeout` | Finite positive seconds when telemetry enabled, default 5 | Same plus 86400-second maximum | Partial | A failed process termination can still delay the attempt; disabled-telemetry behavior tested. |
| Bootstrap before command starts | Python subprocess receives JSON after scope setup | Rust subprocess receives JSON after scope setup | Yes | Windows `wrapper_death_terminates_started_child` and timeout tests. |
| Command argument passing | Direct argv; no shell | Direct argv; no shell | Yes | `clean_and_failed_recovery`; paths and Unicode characters not exhaustively tested. |
| Non-Unicode process arguments | Platform Python argv semantics | Explicitly refused before reservation | No | JSON bootstrap requires Unicode; intentional fail-closed limitation. |
| POSIX process-group creation | New session | New process group | Partial | Both scope descendants; session/controlling-terminal behavior differs. |
| Windows Job Object | Assign bootstrap, kill on job close | Same native APIs | Yes | `wrapper_death_terminates_started_child` on Windows. |
| Windows assignment before command | Bootstrap waits until assigned | Same | Yes | Code and Windows child-started crash test. |
| Child/descendant tracking | Check group or Job active count after successful leader | Check scope; on Windows permit bounded drain before deciding | Partial | Windows behavior intentionally differs; see the detailed change table below. |
| Surviving descendants | Retain reservation; POSIX descendants can continue | Retain reservation; attempt to terminate scope | Partial | Behavioral difference; `timeout_and_descendant_retention`. |
| Whole-tree timeout cleanup | POSIX TERM then leader kill; Windows terminate Job | POSIX group KILL; Windows terminate Job | Partial | Signals differ; wrapper crash on Unix can still leave children. |
| Wrapper crash on Windows | Job kill-on-close; retained reservation | Same | Yes | `wrapper_death_terminates_started_child`. |
| Wrapper crash on POSIX | Child may survive | Same | Unknown | Crash case not explicitly tested on macOS. |
| Interrupt/termination handling | KeyboardInterrupt; scope termination | `ctrlc` interrupt and termination handler; scope termination | Partial | Source inspection; signal handling not independently exercised on Windows or macOS. |
| `--telemetry-csv` | Exclusive new file; refuse overwrite | Same | Yes | Source inspection; duplicate-file behavior not independently tested. |
| Telemetry preflight required | Query before child starts; fail closed | Same | Yes | `telemetry_missing_and_preflight_timeout_do_not_launch`. |
| `--smi-program` | Override query executable | Same | Yes | Fake telemetry integration tests. |
| `--smi-arg` | Repeatable extra query prefix | Same | Yes | `full_utilization_is_not_failure`. |
| Query fields | index, UUID, utilization, used/total memory, temperature, power | Same fields | Yes | Fake telemetry tests; real NVML/driver read not tested. |
| Query parsing | CSV, seven fields, numeric finite nonnegative, nonempty UUID | Same | Partial | Fake success/failure covered; each malformed-value case not separately tested. |
| No GPU telemetry rows | Refuse | Same | Unknown | Source implements; no dedicated test. |
| Periodic samples | Timestamped CSV; flush per query | Same, timestamp in Unix milliseconds | Partial | Different header and timestamp format; `full_utilization_is_not_failure`. |
| Telemetry failure during job | Stop scope and retain reservation | Same | Yes | `later_telemetry_failure_stops_job`. |
| 100% utilization | Does not itself stop job | Same | Yes | `full_utilization_is_not_failure`. |
| Per-query timeout | Stop query and report failure | Same for ordinary-size output; each query gets a process scope | Partial | Termination failure can delay; no GPU-device-recovery guarantee. |
| Query output collection | Captured subprocess stdout pipe | Bounded temporary file, maximum 1 MiB | Partial | `telemetry_descendant_inheriting_stdout_does_not_hold_pipe` tests a query leaving a child; real NVIDIA compatibility remains unknown. |
| Query descendant cleanup | No separate process scope for query | Separate query scope; on Windows permit bounded drain, then terminate if still live | No | Impact on real NVIDIA helpers is not fully established. |
| Samples per all visible NVIDIA GPUs | Emit one row per GPU | Same | Unknown | No multi-GPU fixture or hardware check. |
| Final telemetry shutdown | Stop sampling, join before release | Same | Yes | `full_utilization_is_not_failure`; long sampler exit remains best effort. |
| `--token` match | Required | Same | Yes | `clean_and_failed_recovery`, `competition_and_active_release_refused`. |
| `--confirm-stopped` | Required for recovery | Same | Yes | `clean_and_failed_recovery`. |
| `--recover-stale` | Required when outcome absent | Same | Partial | Source implements; no dedicated recovery test yet. |
| Guard serializes concurrent release | Yes | Same | Partial | Active-release + stale-token tested; deterministic competing double-release/reacquire interleaving not yet forced. |
| Failed recovery on malformed owner | Retain | Same | Unknown | Source fails closed; malformed JSON not tested. |
| Device preflight | Separate pilot helper within reservation | Optional in-reservation NVIDIA UUID, sampled utilization and visible compute-process check | Partial | `preflight_requires_matching_idle_device_before_child_start` plus real NVIDIA query with CPU child. Does not verify LuxCore's selected device or prove absence of graphics/unreported work. |
| Tile watchdog | Separate helper, optionally nested | Not included | No | Must be separately reviewed; no claim of bounded GPU execution. |
| Captured output progress monitoring | Existing project-specific tile watchdog reads stdout | Optional output log and opt-in inactivity deadline | Partial | `progress_watchdog_is_opt_in_and_retains_reservation`; file activity does not prove GPU progress, and cold compilation can be silent. |
| Full hardware/driver fault response | Not guaranteed | Not guaranteed | Partial | Neither implementation can guarantee device recovery. |

The Windows scope-drain change intentionally differs from the earlier Rust candidate:

| Feature / property / behaviour | Earlier Rust guard | Windows drain change | Preserved? | Notes / open questions |
|---|---|---|---|---|
| Successful leader with already-empty scope | Release after checking scope | Release after checking scope | Yes | Windows test suite; no GPU dispatch. |
| Successful leader with short-lived contained descendants | Fail immediately, terminate scope, retain reservation | Recheck the actual Job Object count for up to one second, bounded by the job deadline; release only when empty | No | Deliberate classification and timing change; Windows `short_lived_descendant_is_drained_before_release` and CPU-only Chrome/SwiftShader probe. |
| Successful leader with stubborn descendants | Fail immediately, terminate scope, retain reservation | Fail after the shorter of drain bound and job deadline; terminate scope and retain reservation | Partial | Failure and containment retained; timing and possibly timeout outcome differ. Windows tests include `timeout_and_descendant_retention` and `overall_deadline_terminates_descendant_while_draining`. |
| Telemetry helper with short-lived contained descendants | Reject sample and terminate helper scope immediately | Wait for Job Object to empty, for at most one second and within query deadline | No | Deliberate classification and timing change; `telemetry_short_lived_descendant_drains_before_sample_acceptance`. |
| Telemetry helper with stubborn descendants | Reject sample, terminate helper scope, retain reservation | Reject and terminate after the bounded wait; retain reservation | Partial | Failure preserved, timing differs; Windows `telemetry_descendant_cannot_outlive_query_deadline` and inherited-output test. |
| Windows query deadline | Applied to waiting for helper bootstrap after setup | Starts before helper setup and bounds bootstrap and drain | Partial | A query completing after the deadline is now rejected; Windows `telemetry_launch_cannot_succeed_after_query_deadline` covers expiry before wait. Long process termination is still not guaranteed bounded. |
| Main-job interruption, timeout, telemetry failure | Abort and retain reservation | Still checked while draining | Yes | Source inspection and Windows tests; no claim of driver recovery. |
| Unix process-group behavior | Immediate post-leader scope check using numeric group ID | Same code path | Yes | Not retested in this Windows-only pass. Existing group-ID reuse risk after leader reap remains unresolved; no new Unix drain is enabled. |
| Noncooperating or deliberately escaping processes | Not reliably contained | Not reliably contained | Yes | No broader containment claim. |
