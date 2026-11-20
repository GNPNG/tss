# Sailfish binary-fluid study

This isolated research study tested actual two-component Shan–Chen lattice-Boltzmann simulations using OpenCL on the AMD Radeon Pro 5300M, Intel UHD 630, and an NVIDIA GTX 1650 Ti. It is not an integrated TSS renderer or evidence of photorealism, live 4K performance, variable plate gap, or validated pigment transport. This work addresses [gnpng/org issue 12](https://gitea.gnomatix.com/gnpng/org/issues/12).

## Source and license

[Sailfish upstream](https://github.com/sailfish-team/sailfish), commit `b7140c580e1f4cb406dde947c8d8d5a8a6b2d79e`, identifies its license as LGPL-3.0 in its Python source and package metadata; see the included LGPL and GPL texts. The upstream authors and copyright notices remain in the source and patch. The third-party source tree is not vendored here. `tss-sailfish.patch` is the complete experimental patch against that commit: OpenCL copy API and events, barrier-aware padded work items, binary-kernel `alive` declaration, child process failures, frame snapshot copies, and final-output finite checks. Its scope and behavior require review before use in a product.

The patch also changes the optional Git revision probe in `master.py` to invoke Git with an argument vector and a working directory. An isolated CPU-only reproduction of the former POSIX-shell command produced the same Windows message “The system cannot find the path specified.” seen at the corresponding location in the 320² pilot log; that log did not capture a call stack. The corrected probe handles Git being unavailable or returning an error, without changing the recorded output or claims from the earlier pilot. Four CPU-only Python 2.7 tests passed on Windows; the resulting complete patch passed `git apply --reverse --check` against the patched checkout. The correction itself has not been exercised in a GPU simulation.

## Reproduce on the tested Mac

The study used a separately created Python 2.7 conda environment and PyPI packages. Python 2.7 is end-of-life. `conda-packages.txt` and `pip-freeze.txt` record the observed environment, not a tested production lockfile. It used the conda-forge `python=2.7 numpy scipy sympy=1.3 mako pyopencl pyzmq blosc netifaces execnet` packages, `libcxx=14.0.6`, then PyPI `pytools==2019.1.1`, `Mako==1.1.6`, and `scikit-image==0.14.5`. A fresh install and distribution licensing/security have not been verified.

Clone the source at the recorded revision and apply `git apply tss-sailfish.patch`. Set `PYTHONPATH` to the checkout. On the tested Mac the following executed on OpenCL GPU index 1 (AMD); enumerate devices on the target machine, since indices are machine dependent. Set `PY` to the environment's Python and `SOURCE` to the checkout:

```sh
PYTHONPATH="$SOURCE" "$PY" "$SOURCE/examples/binary_fluid/sc_separation_2d.py" --mode batch --max_iters 1000 --every 50 --seed 12345 --lat_nx 255 --lat_ny 253 --G12 1.0 --output /tmp/phase1 --backends opencl --gpus 1 --noformat_src
PYTHONPATH="$SOURCE" "$PY" "$SOURCE/examples/binary_fluid/sc_separation_2d.py" --mode batch --max_iters 1000 --every 50 --seed 67890 --lat_nx 255 --lat_ny 253 --G12 1.0 --output /tmp/phase2 --backends opencl --gpus 1 --noformat_src
PYTHONPATH="$SOURCE" "$PY" stirred_droplets.py --mode batch --max_iters 1000 --every 20 --seed 12345 --output /tmp/stirred --backends opencl --gpus 1 --noformat_src
PYTHONPATH="$SOURCE" "$PY" test_snapshot_copy.py
PYTHONPATH="$SOURCE" "$PY" logs/test_transfer.py
"$PY" validate_snapshots.py '/tmp/phase1.0.*.npz'
```

`stirred_droplets.py` initializes circles with varied radii once, and evolves both components using Sailfish and a spatial four-roll body force. The two phase separation runs independently initialize the upstream example using different random seeds. Each run represents a two-component simulation; separate runs are not independent pigment species within one physical simulation. Every output NPZ contains `rho`, `phi`, and `v` arrays. Initial conditions, numerical parameters, and simulated evolution are not a calibrated physical model of an overhead-projector plate.

The `logs` directory preserves actual output and transfer/snapshot tests. `snapshot-sha256.txt` records the exact source snapshots used in the experiments. The arrays themselves remain in the temporary experiment directory `/private/tmp/tss-sailfish-out/` on the Mac and are not committed here; they may be removed by the operating system. A hash checks provenance when the original files are available; floating-point results are not promised byte-identical across devices.

The two phase sequences each contained 21 frames with positive, finite component densities and final relative component mass drift below 8.2e-6. The forced-droplet sequence contained 51 frames with positive, finite densities and drift below 9.3e-6. These checks do not establish physical fidelity or temporal/spatial convergence; individual sampled intervals can contain large field changes.

## Windows and limitations

`logs/README.md` and accompanying logs/scripts describe the NVIDIA OpenCL run and the exact local environment. The upstream default multiprocessing path failed on Windows because ZeroMQ rejected an `ipc://` socket. The documented `--debug_single_process` mode completed 1000 steps, with 11 positive, finite component snapshots and relative mass drifts 7.3e-6 and 9.6e-6. This does not resolve the production Windows IPC problem.

The matching NVIDIA/AMD drop snapshots had exactly equal initial densities but diverged after 1000 steps: maximum absolute rho difference 0.01932466 and phi difference 0.002264865. No cross-device acceptance tolerance has been established.

The fixed 2D direct-addressed AB path was exercised on nonmultiple domain dimensions and a wall geometry, without proving all barrier paths or configurations. `--propagate_on_read` produced invalid values and is unsupported in this study; the finite check was changed to fail closed. A separate wall test had small negative density near -0.0059, and a default G12=1.2 phase run had negative density near -0.002: neither is physically validated. The production design still needs fluid, numerical, optical, performance, integration, dependency and licensing review.
