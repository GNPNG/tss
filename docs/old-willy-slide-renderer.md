# Old Willy — merit-rigor result for slide-renderer integration

## Task

Connect an existing confined-fluid solver, optical renderer and interactive compositor into a Windows/macOS desktop application. Keep slide layers independent, calculate within-layer merger behavior, accept pressure and motion input, support live GPU output and offline 4K export, and preserve existing application functionality. Compare correctness, measured execution, maintenance, distribution, security, operating-system support and operating cost. The project owner excludes new Python implementation code.

## Other languages and tools evaluated

### Leading candidates for this specific integration

* **C++ with LuxCore's native C++ API and established solver libraries.** LuxCore's [public C++ API](https://github.com/LuxCoreRender/LuxCore/blob/samples-v2.11.2/include/luxcore/luxcore.h) supports native scene and render-session operations. The project has executed its Windows NVIDIA OpenCL path on an upstream scene ([evidence](../experiments/luxcore-native-study/output/windows-gpu-full-scene-photographic-20261119/EVIDENCE.md)). This makes native C++ a serious candidate for the renderer boundary; it is not evidence that slide optics or cross-platform real-time performance have been established.
* **Rust with `wgpu` and native-library bindings where required.** [`wgpu`](https://github.com/gfx-rs/wgpu) documents the supported native graphics backends; Rust's [ownership model](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) enforces memory-safety rules in safe Rust. The project has exercised `wgpu` separately on both target operating systems. LuxCore interoperation and the complete solver are not proven.
* **Existing JavaScript/WebGL application integration.** The prototype already uses [WebGL](../WebAssets/liquid-renderer.js) and an existing app layer model. This is an existing technology choice to preserve; whether its current renderer can meet the reference is not established. [WebGL's specification](https://registry.khronos.org/webgl/specs/latest/1.0/) defines the browser graphics interface.

These are leading domain candidates, not a substantiated universal ranking. No end-to-end benchmark, complete physical-correctness result, or exhaustive equivalence analysis currently establishes one as best across all criteria. Their coexistence also means language selection for the integration does not select or replace a fluid solver.

### Python versus the leading candidates, per C3 criterion

| Criterion | C++ / Rust / existing WebGL evidence | Python evidence | Result |
| --- | --- | --- | --- |
| Correctness | Rust checks ownership and borrowing in safe code; C++ and GPU/FFI boundaries require testing. The actual physical model remains unvalidated. [Rust ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) | [Python annotations are not enforced by its runtime](https://docs.python.org/3/library/typing.html); external checkers and runtime validation can add checks. Neither language feature validates fluid physics. | **Rust has the cited static ownership/type guarantees; overall physical correctness unknown.** |
| Performance | LuxCore's native NVIDIA test exists, but is an unrelated 480×270 stock scene, not an end-to-end baseline. `wgpu` GPU dispatch exists in experiments. | Python can invoke native C/C++ libraries via its [C API](https://docs.python.org/3/c-api/index.html); native kernels need not run at interpreter speed. Standard GIL-enabled CPython limits concurrent Python bytecode execution; [free-threaded builds exist](https://docs.python.org/3/library/threading.html). No equivalent end-to-end run was measured. | **Unknown for this workload.** No renderer-speed winner is claimed without like-for-like tests. |
| Maintainability | Cargo documents [locked dependency versions](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html); the app's existing JavaScript and LuxCore's C++ remain separate integration surfaces. | Python documents [virtual environments](https://docs.python.org/3/library/venv.html) and type-checker-compatible annotations; native extensions introduce their own ABI/build concerns. | **Unknown overall** without maintaining equivalent implementations. |
| Deployability | Rust produces native executables; C++ and the existing app need platform-specific builds and native dependencies. Tested target packaging is incomplete. [Rust compilation](https://doc.rust-lang.org/book/ch01-02-hello-world.html) | A Python implementation requires an interpreter or packaged runtime plus any native dependencies; its [embedded-distribution documentation](https://docs.python.org/3/using/windows.html#the-embeddable-package) describes one Windows route. No comparable package has been built. | **Unknown overall.** No measured size or reliability comparison. |
| Security | Safe Rust restricts memory-unsafe operations; C++ libraries, shaders, drivers and any `unsafe`/FFI code remain outside those guarantees. [Rust ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) | Python code uses interpreter-managed objects but native bindings have separate safety properties; Python's [C API](https://docs.python.org/3/c-api/index.html) exposes native extensions. Dependency and runtime risk require audit in either case. | **Rust has stronger stated safe-code memory guarantees; overall system security unknown.** |
| Portability | `wgpu` documents Metal, Vulkan and Direct3D backends; installed target-driver and solver support still require tests. [wgpu README](https://github.com/gfx-rs/wgpu) | The [Python downloads](https://www.python.org/downloads/) cover major desktop operating systems; native GPU bindings and solver ABI support are not established for this specific project. | **Unknown at the complete-stack level.** |
| Longevity | `Cargo.lock` records exact resolved dependencies; pinning alone does not ensure 10-year compiler, driver or OS compatibility. [Cargo docs](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html) | Python publishes [release support dates](https://devguide.python.org/versions/); environment/runtime and native-library preservation remain necessary. | **Unknown.** No 10-year reproduction comparison exists. |
| Cost of operation | GPU and CPU usage, memory, thermal load and storage are not measured for an equivalent TSS workload. | Same end-to-end measurements are absent; the fact that Python can invoke native libraries prevents inference from language alone. | **Unknown.** |

### C1–C7 checks

* **C1 — New code:** integration code is needed; this does not imply writing a new fluid or optical solver.
* **C2 — Intrinsic advantage:** Python has not demonstrated an intrinsic language/runtime advantage for this integration. Library availability is a property of those libraries and bindings, not proof of intrinsic superiority.
* **C3 — All eight criteria:** results above include concrete language-property distinctions and multiple unknowns; they do not establish a Python win or tie across all categories.
* **C4 — Ecosystem mandate:** no Python-only platform requirement has been established. A third-party build tool may itself use Python; that does not establish a mandate for TSS project implementation code.
* **C5 — Team-skill contingency:** no team-skill argument is used to justify Python.
* **C6 — Human-cognitive-load argument:** no ease-of-writing argument is used to justify Python.
* **C7 — Verifiability:** claims are limited to cited official documentation and recorded project evidence. End-to-end performance, reliability and long-term comparisons are unmeasured, so no universal ranking is asserted.

## Verdict

Python is not authorized for new project implementation by the owner's explicit instruction, and its superiority is not established under C3 or C7. Preserve the existing JavaScript/WebGL application, investigate native C++ at the LuxCore/solver boundary and Rust/`wgpu` where appropriate, and select integrations based on actual behavior and device measurements. None of these evaluations establishes that the requested renderer is complete or replaces an existing component.

## Honest finding

**n = 0** Python-best cases established among the one integration task evaluated. This is not a conclusion about unrelated tasks or about native third-party tools merely because their build systems use Python.
