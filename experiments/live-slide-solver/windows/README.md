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
