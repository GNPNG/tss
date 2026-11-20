# Basilisk confined-flow study on WSL

This is an isolated test of the unmodified upstream Basilisk Hele–Shaw example on the Windows laptop's `agent-sandbox` WSL 2 distribution. The output is a real CPU simulation of an advected composition field in pressure-driven confined flow. It is **not** TSS, a multi-slide renderer, an established immiscible oil–water or calibrated coalescence model, a 4K result, or a GPU benchmark. Its fingering-front appearance does not match the Mathmos references.

The example treats the material fraction as an advected scalar with variable viscosity. The project's requirements additionally include independent confined slide planes, within-plane coalescence, no cross-plane merging, compression, depth-dependent focus and optical projection; none are established by this example.

## Source and build

- Official [Basilisk source tarball](https://basilisk.fr/basilisk/basilisk.tar.gz), SHA-256 `ab82d30e2825e9632580b7870e6be390fab656962a1667ccc0baac27d36261a5`; included NEWS identifies `20-10-26 (gview)`. No Darcs revision was captured.
- Upstream `src/examples/hele-shaw.c`, SHA-256 `cd46b9e7b4011f40a67bab0f6fb84fb69eb1207fa0ab9c94fbf78634a2cb4250`; source bundle contains GPL-3.0 terms. Distribution and transitive licensing require review.
- Ubuntu 26.04.1 LTS under WSL 2, GCC 15.2.0; single-core build using documented `config.gcc`, `make -k`, then `make`. First build exit 2, second exit 0; logs are preserved on the Windows machine.
- The initial attempt to compile with an absolute source filename failed in `qcc`; compiling from upstream `src/examples` with its relative filename succeeded. Both logs are retained.

## Run and outputs

The unchanged example ran with `OMP_NUM_THREADS=1`, `OMP_THREAD_LIMIT=1`, one-core CPU affinity and a 30-second timeout; exit 124 reflects the imposed deadline. No solver process remained on the subsequent same-name check. No GPU compute work or video encoder ran during the simulation. The NVIDIA GPU was not used for this test.

The Linux distribution had no `ffmpeg` in PATH, so Basilisk's own `output.h` emitted raw PPM streams as it documents in the run log. The composition, velocity and grid streams are retained under `%USERPROFILE%\work\tss-workspace\basilisk-wsl-study\output\` as `f.mp4.ppm`, `v.mp4.ppm` and `level.mp4.ppm`; solver log and scalar field output are in the adjacent `evidence` folder. The field file contains `field-0` and `field-1`, each with columns `x y p f uf.x uf.y`.

The existing Windows FFmpeg executable converted the composition PPM stream with CPU `libx264`, one encoder thread and a chosen presentation rate of 12 fps. It produced 100 frames, 256 × 256, 8.33 seconds. The rate is an encoding choice, not a simulation benchmark. The still is taken at five seconds in that video.

- [Encoded concentration video](output/concentration.mp4), SHA-256 `b8d8f849c5cc34618c8cce881513c5ed633c7813444fd70c643fdd138d9ecbbf`.
- [Concentration still](output/concentration.png), SHA-256 `a8f37a1f98d49096e57abe8fff89150f2ea4d8cd52a1335f3d037acba89e3881`.

The Windows-native FFmpeg was invoked only after the Linux simulation stopped. This experiment did not install or duplicate CUDA or any other system package. The WSL installation's `~/work` symlink was observed resolving to a filesystem loop; the study instead uses the already-existing Windows workspace through the Windows-mounted TSS workspace, without changing that symlink. WSL support here does not establish native Windows packaging or GPU support.

## Single-plane binary-liquid integration test

[`binary_liquid.c`](binary_liquid.c) supplies scene parameters and output events to Basilisk's existing two-dimensional centered Navier–Stokes, VOF, two-phase and surface-tension modules. Both phases occupy the simulated plane; the scene has periodic boundaries, no gravity and no imposed flow. Two initially elliptical phase regions evolve through the solver. The shapes are initial conditions, not hand-animated image assets. The simulation does not model glass walls, a finite plate gap, pressure compression, optical transmission or calibrated coalescence; numerical interface merging must not be represented as verified physical film drainage. The material parameters are test inputs, not measured Mathmos values.

The Windows WSL CPU build used the same pinned Basilisk source and a single CPU core. The bounded run exited 0 and produced 20 P6 RGB grayscale phase-fraction masks, 256 × 256, with corresponding simulation times 0 through 1.9 in 0.1-unit increments. In each mask, white indicates phase fraction 1, black 0 and gray an intermediate sampled fraction. The `frames.csv` also records the phase integral and range: initial integral 0.0818495767597993; last recorded integral 0.081845455940661449. The log contained no NaN, infinity, warning or error text; this is not an exhaustive numerical validation. First and last mask hashes differ, confirming distinct outputs, not establishing quality or calibrated physics.

Source SHA-256 `122b744ad6399bf6632473e5952a8a1be8b517928d6dbf9c30c44c3fb630f2be` and Linux executable SHA-256 `927db9172649065091277bde3ccd8c632fd35fff1dcef82a2ac6027c45790ace`. On the Windows test laptop, masks and timing are under `%USERPROFILE%\work\tss-workspace\basilisk-wsl-study\binary-liquid-output\`; source/build/run evidence is in its parent and `evidence` directories. These are one simulated layer, not a composite or a TSS visual.
