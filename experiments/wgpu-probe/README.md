# Windows GPU compute probe

This standalone test enumerates adapters through `wgpu` and dispatches a small compute shader. It is not connected to TSS, does not simulate fluid or render projector optics, and is not a performance benchmark. `Cargo.lock` records the dependency versions used in the observed run.

## Build and run

On the inspected Windows laptop, from a Command Prompt started in this directory:

```bat
call "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat" -no_logo -arch=x64
set "RUSTC=%USERPROFILE%\.rustup\toolchains\1.88.0-x86_64-pc-windows-msvc\bin\rustc.exe"
rustup run 1.88.0-x86_64-pc-windows-msvc cargo run --release -- vulkan
target\release\tss-gpu-compute-probe.exe dx12
```

The `RUSTC` environment variable selects the installed compiler for this process only. The initial invocation without it failed before compilation because Windows rejected the bare `rustc` shim with `The path cannot be traversed because it contains an untrusted mount point. (os error 448)`. No security settings or system configuration were changed. The commands select the first adapter reported as discrete; an optional second argument selects an adapter by a case-insensitive name substring. Adapter selection is runtime behavior rather than a product requirement for a specific device.

## Observed results

On 2026-11-19, both Vulkan and DirectX 12 enumerated the NVIDIA GeForce GTX 1650 Ti with Max-Q Design as a discrete adapter. The program selected that adapter for each backend, dispatched 64 shader invocations, and verified all 64 integer values read back from the GPU. Vulkan also enumerated Intel Iris Xe; DirectX 12 also enumerated Intel Iris Xe and the Microsoft Basic Render Driver.

`vulkaninfo --summary` separately reported a Vulkan 1.4.363 instance, NVIDIA adapter API version 1.4.351 and Intel adapter API version 1.4.323. It exited successfully after reporting a D3D mapping layer ICD enumeration error and display-extension warnings. `nvidia-smi` reported driver 617.14, CUDA user-mode driver 13.4, 4096 MiB VRAM, and compute capability 7.5. `nvcc` was not on the SSH session PATH; no conclusion is made about toolkit installations outside that PATH.

The machine also has Intel oneAPI 2026.2 under `C:\Program Files (x86)\Intel\oneAPI` and Vulkan SDK 1.4.363.0 under `C:\VulkanSDK`. `icx.exe` was not on the SSH session PATH. This probe does not use or verify oneAPI, and checking PATH alone does not establish whether an SDK is installed. Visual Studio 18 Community x64 build tools and Windows SDK versions 10.0.26100.0 and 10.0.28000.0 were present.

These observations do not demonstrate CUDA execution, other GPUs or drivers, realistic liquid simulation, optical fidelity, 4K throughput, sustained frame rate, or application compatibility. The program uses a fixed small compute workload and does not measure performance.

## macOS Metal check

The same source was built and run on the Intel Mac using the installed Rust 1.88 toolchain:

```sh
CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo +1.88.0 run --release -- metal
```

Metal enumerated the AMD Radeon Pro 5300M as discrete and Intel UHD Graphics 630 as integrated. The program selected the AMD device and verified all 64 computed values. The generated `Cargo.lock` was byte-for-byte identical to the Windows lock file. This is the same limited compute/readback check, not a fluid, fidelity or performance test. Actual output, performance and capabilities on other devices remain to be measured.
