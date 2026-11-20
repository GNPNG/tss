# idpy Metal multiphase probe

This is an isolated numerical experiment, not a TSS renderer. It exercises an existing idpy solver on the Mac's discrete GPU. It has not established the requested oil-and-water material behavior, projection optics, visual quality, interaction, or live and 4K performance.

## Environment and setup

On macOS 15.3.1, Intel x86-64, the Radeon Pro 5300M was Metal device 1 and the Intel UHD 630 was device 0. Source was [idpy `28a09b07c1217abe1d9a435f551cbc16b982986a`](https://github.com/lullimat/idea.deploy/tree/28a09b07c1217abe1d9a435f551cbc16b982986a), MIT license. Its [library-only documentation](https://github.com/lullimat/idea.deploy/blob/28a09b07c1217abe1d9a435f551cbc16b982986a/docs/phase0b-brief.md) says to use a virtual environment and the Metal helper rather than the full initializer. Its [README](https://github.com/lullimat/idea.deploy/blob/28a09b07c1217abe1d9a435f551cbc16b982986a/README.md) documents the base library install. The helper clones a pinned pymetallic revision and applies the two patches supplied by idpy; no other source patches were made.

The following describes the tested base-library setup in an isolated directory. `TSS_REPO` should identify the TSS checkout containing this experiment. The system Bash path was selected because the separate Homebrew Bash installation failed to start on this machine. No `.bashrc` or user Jupyter configuration is required by this path.

```sh
git clone https://github.com/lullimat/idea.deploy.git idpy-study
cd idpy-study
git checkout --detach 28a09b07c1217abe1d9a435f551cbc16b982986a
python3.11 -m venv .venv
.venv/bin/pip install -e .
. .venv/bin/activate
/bin/bash scripts/install-pymetallic.sh
```

The separate optional `.[physics]` installation was also attempted as written in upstream's contributor instructions. On this machine it failed building current `numba` and `llvmlite` source packages when the Homebrew Clang selected by Python failed to load its Z3 library. The successful base-library installation above is a separate documented configuration; the optional physics extra was not installed. Python 3.11.12, idpy 0.1.0 and pymetallic 0.3.1 were used for this experiment.

Check device enumeration and verify the selected device index before running. The scripts currently request `device=1`, the Radeon in the observed run; the index is not a stable hardware identity across hosts.

```sh
.venv/bin/python -c 'from idpy.Metal.Metal import Metal; m = Metal(); print(m.DiscoverGPUs())'
MPLCONFIGDIR="$PWD/.mpl-cache" MPLBACKEND=Agg .venv/bin/python "$TSS_REPO/experiments/idpy-metal/metal_multiphase_probe.py"
MPLCONFIGDIR="$PWD/.mpl-cache" MPLBACKEND=Agg .venv/bin/python "$TSS_REPO/experiments/idpy-metal/metal_multiphase_1000.py"
MPLCONFIGDIR="$PWD/.mpl-cache" MPLBACKEND=Agg .venv/bin/python "$TSS_REPO/experiments/idpy-metal/metal_multiphase_droplet.py"
```

## What was tested

The scripts are new calls to idpy's public API. The flat-interface setup follows the parameters of the upstream [`idpy/LBM/test.py`](https://github.com/lullimat/idea.deploy/blob/28a09b07c1217abe1d9a435f551cbc16b982986a/idpy/LBM/test.py): D2Q9, 32×34 cells, `tau=1`, `SC_G=-3.6`, and densities 0.5 and 1.0. The invocations explicitly select Metal and the Radeon and add 10 or 1,000 evolution steps. The radial probe uses the same grid and material values with `InitRadialInterface(R=8)`, a method also used by the author's [interface-tuning example](https://github.com/lullimat/arXiv-2505.23647/blob/39be0eca60a61e7115ce560b7cbe6ce58d166427/InterfaceTuning.py). These are not unchanged upstream tests or reproductions of the published paper's calibrated experiments.

| Probe | Initial density sum | Final density sum | Relative change | Final finite |
| --- | ---: | ---: | ---: | --- |
| Flat, 10 steps | 662.99999797 | 663.00010335 | +1.589×10⁻⁷ | Yes |
| Flat, 1,000 steps | 662.99999797 | 663.00872332 | +1.316×10⁻⁵ | Yes |
| Radial, 1,000 steps | 645.82173669 | 645.82861754 | +1.065×10⁻⁵ | Yes |

The output identifies the Radeon as the selected Metal device. The inspected idpy Metal path creates a compute pipeline on the selected device, dispatches it through a Metal compute encoder and reads the density field back. The sums are basic observations, not acceptance thresholds or proof of physical accuracy or long-term stability. Combined stdout and stderr are preserved in the corresponding `*-output.txt` files; the local checkout prefix in those files is replaced with `<checkout>`.

The solver converted its data types to FP32 and warned that several constants were emitted as double literals. These warnings need investigation before relying on quantitative results. The recorded elapsed times include setup and compilation but do not establish sustained performance.

## Material-model limits

The inspected `ShanChenMultiPhase` implementation evolves a single density field with one pseudopotential and periodic streaming. Its radial initializer can create a droplet-shaped high-density region. This probe does not establish separately parameterized oil and water, multiple independently controlled dye fields, pairwise interfaces among several liquids, or time-varying plate-gap compression. A wall helper exists in a base class, but the inspected multiphase main loop has not been shown to apply it. Projection light transport and depth of field are outside this solver.
