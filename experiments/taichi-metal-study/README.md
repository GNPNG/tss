# Taichi-LBM3D order parameter and host-array staging

The experiments use Taichi-LBM3D commit `fe49e3f609b2038cbf93c8bd453ffc5c2bf98e4c`. [Upstream source](https://github.com/yjhp1016/taichi_LBM3D/tree/fe49e3f609b2038cbf93c8bd453ffc5c2bf98e4c) · [Upstream MIT license](Taichi-LBM3D-LICENSE)

## Two-component order parameter

The published equation 12 defines the phase as `(rho_r - rho_b) / (rho_r + rho_b)`. Both upstream `streaming3` kernels divide only the blue component by the total. At red density 0.8 and blue density 0.4, the upstream expression gives 0.4666667. Equation 12 gives approximately 0.3333333. The local patch groups the numerator in both kernels. [Yang, Xu and Yang, §2.2, Eq. 12](https://doi.org/10.3390/fluids7080270) · [Dense source, line 605](https://github.com/yjhp1016/taichi_LBM3D/blob/fe49e3f609b2038cbf93c8bd453ffc5c2bf98e4c/2phase/lbm_solver_3d_2phase.py#L605) · [Sparse source, line 606](https://github.com/yjhp1016/taichi_LBM3D/blob/fe49e3f609b2038cbf93c8bd453ffc5c2bf98e4c/2phase/lbm_solver_3d_2phase_sparse.py#L606) · [Order-parameter patch](order-parameter.patch)

For nonnegative component densities the expression is defined when the total density is positive. Upstream initializes each fluid cell with unit total density, where both expressions agree. During streaming it takes the component densities from two separately accumulated fields. Its fluid-cell guard tests geometry and bounds. It does not check the sum of the component densities. A zero-density pair yields `0/0`; the native `f32` test observes NaN. Determining how the solver should report or recover from such a state remains open. [Initialization](https://github.com/yjhp1016/taichi_LBM3D/blob/fe49e3f609b2038cbf93c8bd453ffc5c2bf98e4c/2phase/lbm_solver_3d_2phase.py#L173-L183) · [Streaming](https://github.com/yjhp1016/taichi_LBM3D/blob/fe49e3f609b2038cbf93c8bd453ffc5c2bf98e4c/2phase/lbm_solver_3d_2phase.py#L587-L605)

To reproduce with Git, a POSIX shell and Rust, run the following in a disposable checkout of the upstream commit. Set `TSS_STUDY_DIR` to the absolute path of this study. The first invocation should fail four tests per solver variant.

```sh
sh "$TSS_STUDY_DIR/test-order-parameter.sh" .
git apply --unidiff-zero --check "$TSS_STUDY_DIR/order-parameter.patch"
git apply --unidiff-zero "$TSS_STUDY_DIR/order-parameter.patch"
sh "$TSS_STUDY_DIR/test-order-parameter.sh" .
```

The regression takes the right-hand side of the phase assignment directly from each source file, substitutes scalar variables for the two cell reads, and compiles the arithmetic for native CPU `f32`. The pre-patch failures covered pure red, equal populations, nonunit mixed populations and common density rescaling. After applying the patch, all seven tests passed for each variant, including pure blue, the unit-total initialization round trip, and zero total density. Testing the Taichi-generated kernels, evolved phase mass and interfacial behavior remains open. [Test driver](test-order-parameter.sh) · [Numerical cases](order_parameter_tests.rs)

## Host-array staging

The original small Metal fixture failed before its first fluid step when NumPy's `int64` matrix was passed to a Taichi `f32` field. `host-staging.patch` adds shape, real-numeric, finite, integer-integrality and destination-range validation before creating contiguous destination-typed arrays. It calculates the original matrix inverse in NumPy's original precision, then converts the result once for the `f32` device field. It also explicitly stages geometry as `int8` and phase values as `float32`; it preserves the original geometry normalization and Fortran-indexed reshape. Invalid input is rejected rather than clamped. The patch does not change the numerical solver or its phase equation.

## Reproduce in an isolated checkout

The following uses Python 3.11.12, Taichi 1.7.2, NumPy 1.26.4 and pyevtk 1.7.0, as tested on the Intel macOS 15.3.1 machine. Select and review the environment and dependencies appropriate for your platform. `TSS_STUDY_DIR` must point to this directory, and the destination clone and fixture directories must not already exist.

```sh
git clone https://github.com/yjhp1016/taichi_LBM3D.git taichi-study
cd taichi-study
git checkout --detach fe49e3f609b2038cbf93c8bd453ffc5c2bf98e4c
python3.11 -m venv .venv
.venv/bin/pip install taichi==1.7.2 numpy==1.26.4 pyevtk==1.7.0
git apply "$TSS_STUDY_DIR/host-staging.patch"
cp "$TSS_STUDY_DIR/staging.py" 2phase/staging.py
cp "$TSS_STUDY_DIR/test_staging.py" "$TSS_STUDY_DIR/test_phase_formula.py" "$TSS_STUDY_DIR/test_roundtrip.py" 2phase/
(cd 2phase && ../.venv/bin/python -m unittest -v test_staging test_phase_formula)
(cd 2phase && TSS_TEST_BACKEND=cpu ../.venv/bin/python test_roundtrip.py)
(cd 2phase && TSS_TEST_BACKEND=metal ../.venv/bin/python test_roundtrip.py)
.venv/bin/python "$TSS_STUDY_DIR/prepare_fixture.py" 2phase/lbm_solver_3d_2phase.py /tmp/taichi-fixture-cpu --backend cpu
.venv/bin/python "$TSS_STUDY_DIR/prepare_fixture.py" 2phase/lbm_solver_3d_2phase.py /tmp/taichi-fixture-metal --backend metal
(cd /tmp/taichi-fixture-cpu && "$OLDPWD/.venv/bin/python" -u solver.py)
(cd /tmp/taichi-fixture-metal && "$OLDPWD/.venv/bin/python" -u solver.py)
```

For the final two commands, the subshell starts in the checkout and uses `OLDPWD` after changing to the fixture directory. The fixture generator makes an all-open 32³ geometry and spherical phase initialization; it defaults to **zero** evolution steps and saves initialized fields to `fields.npz`. It refuses an existing destination directory. Specify `--steps 2` only for a separate bounded collision-kernel diagnostic; it may compile for several minutes, has not completed here, and is not a performance benchmark. The full upstream 131³ case was not rerun after this staging change: the separately observed Taichi 1.0.2 padded field allocation and stride overflow remain unresolved.

## Results and limits

- Seven host tests passed: four for staging validation and three characterizing the source/paper formula discrepancy. Four small destination-typed upload/readbacks passed on Taichi 1.7.2 for both CPU and Metal.
- The 32³ zero-step fixture initialized on CPU and Metal. All elements of the five inspected fields matched bit-for-bit and were finite: `rho` sum 32768, `rho_r` sum 2176, `rho_b` sum 30592, `psi` sum −28416 and `solid` sum 0.
- Taichi 1.7.2 identifies `arch=metal` but did not print an adapter identity. Its [source calls `MTLCreateSystemDefaultDevice()`](https://github.com/taichi-dev/taichi/blob/v1.7.2/taichi/rhi/metal/metal_device.mm#L1172-L1175); a separate native Metal probe found this Mac's default device to be the Radeon Pro 5300M. Thus Radeon selection is an inference, not a per-process device confirmation.
- A two-step 32³ Metal test was stopped at about three minutes while compiling the first collision kernel. A sampled call stack showed Taichi's `Program::compile_kernel` and `WholeKernelCSE`. An extended repeat was stopped at the eight-minute cutoff while still reporting `BEFORE collision`; no fluid evolution finished. During that repeat process resident memory was observed up to about 1.1 GiB and macOS reported at least 56% system-wide memory free at the checks. Two earlier 1.0.2 full-solver attempts were also stopped after roughly two minutes each. The processes exited and no compiler child remained. Startup time here is not a sustained frame-time measurement.
- The historical staging trials used the upstream phase expression. The code revision used to generate the authors' published benchmark results is unknown.
- No Windows, CUDA, sustained-step, mass-conservation, interfacial-tension, variable-gap compression, independent-pigment, visual-quality, or 4K tests passed as part of this study. A real solver integration requires those checks, including supported field layout and memory accounting on both target devices.

Further evidence and root-cause analysis are recorded in [issue #11](https://gitea.gnomatix.com/gnpng/org/issues/11).
