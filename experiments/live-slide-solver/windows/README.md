# Windows x64 live-solver build

The CPU worker advances each seeded slide in a separate process. The Rust editor and CImg bridge use the x64 MSVC toolchain. Basilisk uses POSIX facilities including memory mapping and resource usage. The worker build translates Basilisk C to C99 using the project's existing WSL qcc and compiles the result into a Windows PE with Cygwin GCC. The worker's Cygwin DLL is installed beside its executable. WSL and the compiler participate in the build only. [Basilisk qcc](https://basilisk.fr/src/qcc.c) · [Basilisk build requirements](https://basilisk.fr/src/INSTALL) · [Cygwin](https://cygwin.com/)

## Build

In PowerShell, provide the path to the checked Basilisk source tree inside WSL and the Windows path of a Cygwin x64 installation containing gcc-core. The output directory must be new. The optional `-WslDistribution` argument selects a non-default build distribution.

```powershell
.\scripts\build-windows-solver.ps1 -BasiliskSourceWsl $BasiliskSourceWsl -CygwinRoot $CygwinRoot -OutputDirectory .\target\solver-bin
```

Point `CIMG_INCLUDE` at the directory containing the pinned CImg 3.6.0 header. Its SHA-256 is `d769ec34013eba8f19bdc7eb42f4f416159b1ca3840a2b4a2e4aa1a14d9fa85d`. The editor's normal Rust 1.88 MSVC, Vulkan SDK build tools and Go performer build apply. The feature compiles in the Rust runtime and C++ optical bridge. [CImg source revision](https://github.com/GreycLab/CImg/tree/01230c15cfe23d5ffd9b1d1ba05aefdbcac6ab00)

```powershell
$env:CARGO_BUILD_JOBS = '1'
cargo build --locked --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol,live-solver
go -C performer build -trimpath -o ../src-tauri/target/release/ .
.\scripts\package-windows.ps1 -Tag v0.1.0-local -OutputDirectory .\target\windows-package -LiveSolverDirectory .\target\solver-bin
```

The explicit solver directory includes the Windows worker, its POSIX runtime and their supplied licence notices in the local ZIP. The packager's existing two-executable mode remains available when the option is omitted. Release distribution also requires review of all source and licence obligations for the selected build.

## Windows measurements

On the Windows x64 laptop, Rust 1.88.0/MSVC checked the complete feature-enabled desktop editor. The independently linked Rust optical executable exposed CImg's Shell32 dependency. After adding the target-specific link declaration, the linker produced both the cell and optical test executables. Cygwin GCC 14.4.0 compiled the worker against runtime 3.6.12. The numerical test launched the relocated worker via Windows process pipes with only the Windows system directories on PATH.

At 64 × 64 per cell, the reference and driven pairs used seed 83724. All 18 rear fields matched byte for byte. The driven front differed in 419 samples by tick 18. Across the full domain the maximum relative phase-volume drift was 2.75 × 10⁻⁶ (front) and 3.41 × 10⁻⁶ (rear). The recorded median single-cell step was 36.495 ms. [Cell measurements](cell-check.txt)

The 32 × 32 pair feeding the 132 × 132 experimental optical atlas completed 24 measured ticks. The median time for both serial solver steps plus CImg's two-channel distance transform was 21.288 ms and the 95th percentile was 33.286 ms. The measurement excludes GPU presentation. [Optical measurements](optics-132.txt)

## Local package verification

The optimized MSVC editor and Go performer linked on Windows x64. The package check extracted the local ZIP into a new directory and verified all ten expected files, including the Cygwin runtime and licence notices. With only Windows system directories on PATH, the verifier launched the packaged worker and repeated the 64² gesture-control test. With the production settings of two 32² cells and a 176² CImg field, the pair completed 48 measured ticks. Median end-to-end CPU time was 15.925 ms, p95 18.109 ms and maximum 30.544 ms. GPU presentation and editor startup were outside this Windows test. [File hashes and timings](package-check.txt)

The local review file `The-Sacred-Swirls-v0.1.0-live-solver-local-Windows-x64.zip` contains 82,582,429 bytes. Its SHA-256 is `dd30c3c75868fbfd412d320a9b6e9f8612691ef470fdfb920ab84fa6b273736e`. The quickstart links to source commit `171d9fe17e22daf6f8f1fed974d16772d93562bc`.

To repeat the archive test, use the x64 MSVC developer shell with the CImg directory set as described above. The fixture runners operate on the CPU and use a new temporary extraction directory.

```powershell
cargo build --release --locked -j 1 --examples --features cimg-optics --manifest-path experiments/live-slide-solver/Cargo.toml
$examples = 'experiments/live-slide-solver/target/release/examples'
.\experiments\live-slide-solver\windows\check-package.ps1 -Archive $Archive -CellCheck "$examples/check.exe" -OpticalPair "$examples/optical_pair.exe"
```
