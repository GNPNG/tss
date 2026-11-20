# SPlisHSPlasH upstream study

This is an evaluation of the native C++ SPlisHSPlasH source at commit `dba6b26b9387ab6bdd187d6df0a7c9d490d03500`; it is not a TSS renderer or an oil-projection model. The source is obtained from <https://github.com/InteractiveComputerGraphics/SPlisHSPlasH>. Its MIT license is retained in `LICENSE.SPlisHSPlasH`; the upstream checkout contains additional licenses for its dependencies.

## Backend and physical scope

The upstream [README](https://github.com/InteractiveComputerGraphics/SPlisHSPlasH/blob/dba6b26b9387ab6bdd187d6df0a7c9d490d03500/README.md) describes CPU-parallelized SPH solvers and optional GPU neighborhood search via cuNSearch. GPU neighborhood search is not a full GPU fluid solver. cuNSearch requires NVIDIA CUDA; it does not establish AMD Radeon support. The CPU CompactNSearch build tested here uses OpenMP; no GPU program was run in this study.

Multiple fluid models alone do not establish calibrated oil-water interface physics. In particular, the [Akinci surface-tension implementation](https://github.com/InteractiveComputerGraphics/SPlisHSPlasH/blob/dba6b26b9387ab6bdd187d6df0a7c9d490d03500/SPlisHSPlasH/SurfaceTension/SurfaceTension_Akinci2013.cpp) computes its fluid-neighbor surface-tension terms within the same phase and handles solid-boundary adhesion separately. Whether any of the provided methods provides suitable pairwise interfacial tension between two fluids needs further verification; visual fidelity and interactive performance have not been established.

## macOS CPU build

Environment examined: Intel macOS, Apple Clang 17.0.0, CMake 4.4.5, Homebrew libomp 22.1.8. The pinned source includes Eigen 3.4.0. The parent environment selected Homebrew LLVM 22.1.8_2 via `CC` and `CXX`, but its compiler could not start because the installed z3 5.4.1 did not supply the requested `libz3.5.1.dylib`. This remains unresolved. Apple's toolchain was selected explicitly for this independent test, consistent with upstream's [macOS CI](https://github.com/InteractiveComputerGraphics/SPlisHSPlasH/blob/dba6b26b9387ab6bdd187d6df0a7c9d490d03500/.github/workflows/build-mac.yml).

Homebrew Eigen 5.0.1 was installed during this study even though the upstream source already bundles Eigen; the pinned upstream finder did not recognize Eigen 5's version macros. The build explicitly selects the bundled Eigen. It was subsequently uninstalled after `brew uses --installed eigen` showed no installed dependents, restoring the prior package state. The PBD external dependency also selected it and failed configuration; after removal, its stale Eigen cache entry was cleared and the dependency found its bundled Eigen.

From this directory:

```sh
git clone --no-checkout --filter=blob:none https://github.com/InteractiveComputerGraphics/SPlisHSPlasH.git vendor/upstream
git -C vendor/upstream checkout --detach dba6b26b9387ab6bdd187d6df0a7c9d490d03500
CC=/usr/bin/clang CXX=/usr/bin/clang++ cmake -S vendor/upstream -B build-git \
  -DCMAKE_BUILD_TYPE=Release -DUSE_PYTHON_BINDINGS=OFF \
  -DNeighborhoodSearch=CompactNSearch \
  -DEIGEN3_INCLUDE_DIR="$PWD/vendor/upstream/extern/eigen" \
  -DCMAKE_PREFIX_PATH=/usr/local/opt/libomp
CC=/usr/bin/clang CXX=/usr/bin/clang++ cmake --build build-git \
  --target KernelTests ReadWriteStateTests --parallel 2
```

The prefix above is specific to this Intel Homebrew installation. Configure detected OpenMP 5.1 and completed. Upstream obtains further dependencies at the pinned revisions specified in its CMake files. Configure evidence, including failed attempts, is retained in `evidence/`; raw clone and ongoing build logs are under the locally retained, git-ignored `output/` directory.

## Windows toolchain investigation

On the Windows test laptop, CUDA compiler executables reported versions 12.9.86, 13.4.59, and 13.5.37 from standard toolkit directories. Visual Studio Installer's `vswhere` reported Visual Studio 18 Community with the C++ tools component. Toolkit compatibility with that compiler and with cuNSearch was not tested; no GPU execution or Windows build was performed here.

The first two-target build produced `KernelTests` but failed building `ReadWriteStateTests` when the PBD external project selected Eigen 5. After restoring the package state, PBD's stale `EIGEN3_INCLUDE_DIR` cache entry was removed through CMake's `-U` option, and its own configuration found its bundled Eigen. The state-test build was resumed after an interrupted turn and completed. Logs for the initial failure and PBD reconfiguration are retained in `evidence/`.

`gtimeout 120s env OMP_NUM_THREADS=1 ./vendor/upstream/bin/KernelTests` passed: **34 assertions in 12 test cases**, 0.84 seconds wall time, measured using `/usr/bin/time -p`. This test exercises SPH kernels; it does not validate a complete oil-water simulation or a 4K renderer.

`gtimeout 120s env OMP_NUM_THREADS=1 ./vendor/upstream/bin/ReadWriteStateTests` passed: **56 assertions in one test case**, 1.30 seconds wall time measured using `/usr/bin/time -p`. The upstream test disables the GUI and advances a single-fluid scene for five steps before checking a saved and restored state. This verifies that bounded CPU simulation/state path; it does not validate multiphase interfacial tension, rendering, or live performance. The resumed two-job state-test build took 376.97 seconds on its final invocation; earlier interrupted build time is additional. Compiler warnings in the retained log include format-string warnings in upstream GUI code.

## Relevance to liquid between glass plates

The project geometry is liquid constrained between glass plates, with the possibility of moving or compressing the plates. The upstream library has 2D and 3D fluid solvers, distinct fluid models, rigid boundary models, and boundary-adhesion terms. These are potentially useful components, but the tests above do not validate a confined oil-and-water film. The pinned [Akinci implementation](https://github.com/InteractiveComputerGraphics/SPlisHSPlasH/blob/dba6b26b9387ab6bdd187d6df0a7c9d490d03500/SPlisHSPlasH/SurfaceTension/SurfaceTension_Akinci2013.cpp) applies its fluid cohesion to same-phase neighbors and boundary adhesion separately; this is not evidence of pair-specific oil-water interfacial tension or a calibrated dynamic contact angle. The pinned [PBD boundary simulator](https://github.com/InteractiveComputerGraphics/SPlisHSPlasH/blob/dba6b26b9387ab6bdd187d6df0a7c9d490d03500/Simulator/PositionBasedDynamicsWrapper/PBDBoundarySimulator.cpp) explicitly reports that dynamic boundaries are unsupported in its 2D simulations. It does not establish a moving-gap model for 2D operation.

Two-phase Hele-Shaw and lubrication research is relevant to compare when the actual plate gap, lateral scale, viscosity, speeds, and wetting regime justify the corresponding approximations. Examples include a [time-dependent-gap two-phase problem](https://arxiv.org/abs/1711.02533), [wetting-film effects at an immiscible interface](https://arxiv.org/abs/2103.05206), and a [volume-of-fluid formulation for two immiscible liquids in a shallow cell](https://arxiv.org/abs/1308.6629). These references motivate evaluation; they do not prove that any one model captures the user's physical apparatus. Neither SPH nor a depth-averaged model is selected by this study.

## Homebrew LLVM failure, still open

The installed Homebrew LLVM 22.1.8_2 and flang 22.1.8 receipts both identify Z3 5.1.0 as a runtime dependency. `otool` reports that `libLLVM.dylib` requires `/usr/local/opt/z3/lib/libz3.5.1.dylib`, compatibility version 5.1.0. Only Z3 5.4.1 was observed in the Cellar and `/usr/local/opt/z3` was absent. The failed compiler invocation and receipt/linkage inspection are preserved in `evidence/`. This is independent of the passing Apple Clang builds.

The official [Homebrew Z3 5.1.0 formula](https://github.com/Homebrew/homebrew-core/blob/f28b3ef5ee1d6a6b56cb20ecc9b40692e2ed66f2/Formula/z/z3.rb) was located at its original core commit; Homebrew documents `version-install` and `extract` for historical versions. Such an installation uses a versioned formula and separate opt prefix; whether Homebrew can restore the exact unversioned path required by the installed binaries while preserving Z3 5.4.1 and its expected use has not been established. The examined local cache did not contain a Z3 5.1 archive. The historical formula lists an Intel Sonoma bottle but no Intel Sequoia bottle; this host is macOS 15.3.1 (Sequoia).

A `brew upgrade --dry-run llvm` predicted LLVM 22 to 23, four dependency upgrades, one dependency install, and five formulae built from source. Current official `llvm@22` disables Z3 support, changing behavior, so neither path has been applied. No forced symlink, dynamic-loader override, package downgrade, or global compiler substitution is a verified repair. The LLVM failure remains open pending a supported fix that preserves functionality and passes the original compiler invocation and dependent checks.
