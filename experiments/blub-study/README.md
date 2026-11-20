# Blub evaluation

Source: [Wumpf/blub](https://github.com/Wumpf/blub), commit
`939f346cf3a8e2173834d46fe6979ae42a0d4cc2` (2022-12-17), MIT license.
The repository is an application and research experiment, not a documented
drop-in multiphase-liquid library.

## What the source implements

- GPU APIC fluid simulation with implicit density projection, a preconditioned
  conjugate-gradient pressure solver, and voxelized solid boundaries. The
  implementation and limitations are described by the author in the upstream
  README.
- Screen-space surface reconstruction and shading, not a demonstrated
  photorealistic projector optics renderer.
- `src/scene/mod.rs` describes one `FluidConfig` and creates one `HybridFluid`.
  `shader/simulation/density_projection_gather_error.comp:108-110` uses a
  unit particle mass and a fixed rest density. Per-liquid viscosity, material
  identity, interfacial tension, and coupled oil/water phases have not been
  established for this implementation. Separate colored appearances alone
  would not establish these interactions.
- `src/main.rs:76` explicitly requests Vulkan. The device request requires
  several features including conservative rasterization and timestamp queries.
  It asks for a high-performance, window-compatible adapter; it does not select
  an adapter by UUID. A build does not verify GPU feature support or device
  selection.

## macOS status

The upstream README says Mac is not working. In [issue 26](https://github.com/Wumpf/blub/issues/26),
the author closed the issue in 2021, stating that additional features were not
supported on Metal; earlier comments describe image atomic and shader
translation problems. This is the upstream account for that implementation,
not a test on the Radeon Pro 5300M or a statement about all Vulkan/Metal software.
No GPU dispatch was performed on the Mac for this evaluation.

## Windows build, CPU compilation only

The Windows test laptop used x86-64 MSVC. Repository checkout:
`%USERPROFILE%\work\tss-workspace\blub-study\source`.
Upstream requires Git LFS and specifies `cargo run`; this evaluation uses
`cargo build --locked`, which compiles without launching the application.

The first build invoked `rustup run stable-x86_64-pc-windows-msvc cargo build
--locked`. Cargo could not execute the `rustc.exe` proxy in
`%USERPROFILE%\.cargo\bin`: Windows returned error 448, "The path cannot be
traversed because it contains an untrusted mount point." The proxy was observed
to be a symbolic link to `rustup.exe`. `rustup which rustc` returns an actual
installed compiler, and that binary runs successfully. The documented Cargo
[`RUSTC` setting](https://doc.rust-lang.org/cargo/reference/environment-variables.html)
was used for the build process only; no system security or
configuration change was made:

```powershell
$env:RUSTC = (rustup which rustc)
Set-Location "$env:USERPROFILE\work\tss-workspace\blub-study\source"
rustup run stable-x86_64-pc-windows-msvc cargo build --locked
```

Cargo reports `unused manifest key: patch.crates-io.wgpu.commit` and that the
`wgpu` Git patch is not used in the crate graph. The checked-in `Cargo.lock`
specifies the crates.io `wgpu` 0.9.0 and an unused Git patch for `wgpu` 0.10.1
at `fc1cb5eb20ebe06bf3954b79dd682b65406f113c`. No manifest or dependency was
changed in this evaluation. This discrepancy matters when interpreting build
results and must be resolved from upstream evidence before integration.

Build result: **success**, 3 minutes 10 seconds, with Rust/Cargo 1.98.1. In
addition to the patch warnings above, Rust emitted five warnings: unused struct
and field, two lifetime syntax warnings, and a mutable reference to a mutable
static. The successful build is not evidence that any GPU feature or rendering
works. The checkout remained unchanged. The debug binary is
`%USERPROFILE%\work\tss-workspace\blub-study\source\target\debug\blub.exe`,
SHA-256 `1990896a0ff826cab26f522b32f68e6c1ac783ad74d247ab60e364a096ec7180`.
No GPU application execution was performed.
