# Basilisk confined-liquid candidate: CPU study

This evaluates the upstream C-based [Basilisk](https://basilisk.fr/) framework on the Intel Mac, without dispatching GPU work. It does not establish the intended oil-projector renderer, a validated glass-slide model, an interactive rate, NVIDIA/Windows support, or selection of the Mac Radeon.

## Source and build

The [upstream installation instructions](https://basilisk.fr/src/INSTALL) document a version-controlled Darcs checkout and a tarball containing the same source. Darcs was not present, so the official tarball was obtained from `https://basilisk.fr/basilisk/basilisk.tar.gz` on 2026-11-19 and retained under the ignored `vendor/` directory. Its SHA-256 is `ab82d30e2825e9632580b7870e6be390fab656962a1667ccc0baac27d36261a5`; the included NEWS identifies release `20-10-26 (gview)`. This hash pins the examined snapshot; it is not a Darcs revision or an independently verified upstream checksum.

The source distribution includes [GPL version 3](https://basilisk.fr/src/COPYING). Review the licenses and provenance of the exact code, generated output and bundled components before integration or distribution.

The [upstream macOS installation guide](https://basilisk.fr/sandbox/INSTALL_MACOS) specifies `config.osx` and Apple's C toolchain. The build used macOS 15.3.1, Xcode Apple Clang 17, and GNU Make 3.81. An unsupported inherited `C.UTF-8` locale caused archive listing and hashing problems; commands used process-local `LC_ALL=C`. `CC=/usr/bin/clang` and `CXX=/usr/bin/clang++` selected the documented Apple toolchain without changing shell or system settings.

The first `make -j2` failed while a subdirectory tried to generate dependency files before `qcc` existed. The [upstream installation page](https://basilisk.fr/src/INSTALL) explicitly documents `make -k` followed by `make`. Following that sequence produced the qcc compiler and the second invocation exited zero. The initial failure and both subsequent logs are retained under ignored `evidence/`.

## Hele–Shaw example

The unchanged upstream `src/examples/hele-shaw.c` was compiled with `qcc -Wall -O2 ... -lm`, using the CPU default grid. Its source SHA-256 is `cd46b9e7b4011f40a67bab0f6fb84fb69eb1207fa0ab9c94fbf78634a2cb4250`. The example uses a viscosity-dependent advected composition scalar and variable-coefficient pressure equation. It does not establish a sharp immiscible interface, pairwise surface tension, glass wetting, or changing plate-gap dynamics.

It was **not executed**: its three movie output streams invoke separate FFmpeg `libx264` encoders, and an explicit bound on all encoder threads had not been established. A numerical test without movie generation was selected instead.

## Surface tension and wetting test

The unchanged upstream `src/test/sessile.c` ([documentation](https://basilisk.fr/src/test/sessile.c), SHA-256 `3c5bc05073ad13a2e9b746214c9aec113f4369da90f76850715963dbf1d861ae`) tests a 2D sessile drop with VOF interface reconstruction, surface tension and an imposed solid contact angle. It is a separate test and not a Hele–Shaw cell. Its complete upstream sweep is 15° through 165° in 15° steps.

Using `OMP_NUM_THREADS=1`, `OMP_THREAD_LIMIT=1` and a 20-second GNU `timeout` with a two-second kill-after interval, the stock binary timed out with exit 124 after completing only the 15° case. It emitted interface coordinates and `64 15 16.119 0.0109`: grid size, angle, normalized radius, and reported curvature standard deviation. The analytical expression printed in upstream documentation evaluates to approximately 16.31718 at 15°, a difference of about 1.2% for the reported rounded result. No upstream acceptance tolerance has been established; this is a partial observation, not a test pass.

A longer one-thread run with a 240-second timeout terminated after five completed angles (15°–75°) with exit 136. The macOS diagnostic report identifies `SIGFPE` at a square-root instruction in `statsf`, called by the test's end event. Basilisk enables invalid-operation and divide-by-zero traps on macOS. The original Apple Clang 17 `-O2` disassembly computes the square root before masking the source conditional result; the [Clang manual](https://clang.llvm.org/docs/UsersManual.html#controlling-floating-point-behavior) documents that floating-point exception behavior defaults to ignore. An unmodified-source diagnostic build with `-ffp-model=strict` branches around the square root as expressed in the source, while retaining traps. That run reached its 240-second timeout after completing nine of eleven angles, including 90° where the original build failed. The square-root operand at the crash was not captured, and neither the exact trigger nor a complete fix has been proven.

The candidate configuration-level repair [config-osx-trapping-math.patch](config-osx-trapping-math.patch) adds Clang's `-ftrapping-math` to `config.osx`'s C99 flags, which also become part of `qcc`'s compiler command. It requests the documented strict exception behavior without disabling Basilisk's traps. On a clean extraction of the same archive, the patch applied cleanly and the documented two-pass build succeeded. The resulting `qcc` embeds the flag and recompiles the unchanged test with a branch guarding the square root.

A one-thread full sweep of the patched build was stopped deliberately after three angles, using SIGTERM, because the host's one-minute load rose to 16.92 on 12 logical CPUs with other applications active. Its exit 143 reflects that interruption, not an observed simulation failure. A complete sweep of the actual configuration patch remains unverified. This is a local candidate patch, not an upstream fix or evidence of other configurations. The original crash report and run outputs are preserved under ignored `evidence/`.

The captured logs and generated binary/outputs remain locally under ignored `evidence/`. Source changes have not been made.

## Viscoelastic pressure-step reference

The unmodified upstream Oldroyd-B planar Poiseuille case ran through all three grid levels on the Intel Mac CPU. The input is a suddenly imposed axial pressure gradient. The observable is the overshoot and relaxation of centreline velocity. This provides an independent rheology check for the slime investigation. The live wheel still uses its original oil trajectories. [Upstream source and analytical solution](https://basilisk.fr/src/test/poiseuille-oldroydb.c) · [All 153 samples](oldroyd-b-startup.dat)

| Grid | Sampled peak at t/λ = 1.2 | Peak of analytical samples | Largest absolute error across samples |
|---|---:|---:|---:|
| 16 × 16 | 2.55382 | 2.55066 | 0.00656 |
| 32 × 32 | 2.55180 | 2.55066 | 0.00183 |
| 64 × 64 | 2.55113 | 2.55066 | 0.00614 |

The five columns are t/λ, computed and analytical centreline velocities in units of the mean steady velocity, grid level, and the model's L² field. The error across the three grids is nonmonotonic. At t/λ = 10 the finest grid gave 1.50607 against 1.50387. The test uses a single relaxation time and a solvent/total viscosity ratio of 1/9. A material preset for slime still needs measurements of the particular formulation.

To reproduce with the study's Basilisk compiler, copy the pinned source into a fresh working directory and run the following there. Its SHA-256 is `98f18af6705b976349f69b6f78d939273aeda5ca5982fb9719d1ba00bc36dd47`.

```sh
cp "$BASILISK/test/poiseuille-oldroydb.c" .
"$BASILISK/qcc" -O2 -Wall poiseuille-oldroydb.c -o upstream-poiseuille -lm
OMP_NUM_THREADS=1 ./upstream-poiseuille >flow.out 2>oldroyd-b-startup.dat
```
