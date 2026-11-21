# Procedural preview at fixed zoom

An Intel Mac running macOS 15.3.1 displayed the RC.1 editor and working editor successively in a single WKWebView. The harness selected three liquid tracks through the real editor controls, applied detail scale to each, released the controls, and measured the next 2.2 seconds after six warm-up frames. The canvas was 1280 × 720. From bottom to top, the track themes were Orchid, Ember and Twilight with opacities of 1, 0.62 and 0.47. The animation ran on the editor clock with no imported audio. WebKit reported “Apple GPU,” which does not identify the physical adapter.

| Fixed detail scale | RC.1 callbacks/s | Working callbacks/s | RC.1 / working p95 interval | RC.1 / working maximum interval | RC.1 / working mean time inside frame callback |
|---|---:|---:|---|---|---|
| 0.4× | 60.10 | 59.95 | 19 / 18 ms | 22 / 18 ms | 13.61 / 0.13 ms |
| 1× | 58.29 | 59.01 | 23 / 18 ms | 36 / 64 ms | 13.84 / 0.13 ms |
| 2× | 58.08 | 59.95 | 20 / 18 ms | 42 / 20 ms | 14.04 / 0.10 ms |

The working path used one WebGL context. RC.1 used three. At 1× the working version had one 64 ms callback gap. Callback execution time counts JavaScript submission and any synchronous waits. GPU completion and screen scanout were outside this timing interval. The six measurements are single short runs on this Mac. Windows/WebView2, native slide rendering, live audio and 4K presentation remain separate cases. [Run statistics and exact source hashes](results.json)

## Motion at equal times

A second run read 80 RGB pixels across row 360 **in the same callback** immediately after each production WebGL draw. It checked all three layer submissions, final canvas output and the corresponding pixels from RC.1. Each zoom had eight identical-time draws, twenty 1/60 s steps and four returns to the original instant.

| Fixed scale | Largest mean RGB difference after returning to the same instant | Largest adjacent-frame mean difference, working | Largest RC.1 / working canvas mean difference |
|---|---:|---:|---:|
| 0.4× | 0 | 0.308 | 0.238 |
| 1× | 0 | 0.408 | 0.221 |
| 2× | 0 | 0.321 | 0.213 |

Differences are in 8-bit RGB code values. At the sampled pixels, the opaque base matched byte-for-byte across the two editors. The working canvas matched its final framebuffer byte-for-byte. Shader inputs, draw order and opacities matched. The sampled base and final images repeated exactly at the fixed timestamp. [Scanline diagnostic](diagnostic-results.json)

The first run also scaled the preview into a 96 × 54 Canvas2D copy for comparison. Both versions showed a jump in that readback at 1×. The jump disappeared in the synchronized framebuffer test. The two tests differ in sampling and synchronization, so the first result cannot identify a shader discontinuity or establish what appeared on the physical display. Adding a synchronous readback to the production render loop would change the timing being studied.

## Repeat on macOS

With Node, Apple Command Line Tools and the built project GPU guard available, run from the repository root:

```sh
experiments/preview-cadence/run.sh
experiments/preview-cadence/run.sh diagnose
```

Set `TSS_CADENCE_NODE` to select a Node executable. The default comparison commit is `6ca9c32083b88423345c45d4f0ae2eb237cd17fa`. Each command stages both editors in a fresh ignored `target/preview-cadence/run.*` directory, serves static files on a random loopback port, and launches one visible WebKit window under the existing Mac reservation. The runner prints its output directory. The cadence report and the diagnostic report can also be regenerated from its `host.log` with `report.mjs <directory> <baseline-commit>` or `diagnostic-report.mjs <directory>` via Node.

During the first diagnostic, the host delivered both complete JSON records and then waited for an AppKit event after requesting shutdown. Its idle process was terminated and its reservation released after checking the job tree and accelerator. The harness now posts a wake event at shutdown. The revised shutdown path compiled. It has not been run again on this Mac.
