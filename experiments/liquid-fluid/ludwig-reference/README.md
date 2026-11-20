# Ludwig numerical reference

This is a reproducibility record for a small coupled two-phase CPU test, **not the GPU-first TSS renderer**. Ludwig is an established BSD-3-Clause physics code; no Ludwig source is bundled here. Its output is not yet connected to dye transport or projector optics and has not been compared visually with the supplied liquid references.

Pinned source: <https://github.com/ludwig-cf/ludwig/tree/78d83c5151ff1d4c4201a96b266a08e97047cae6>; license in `LICENSE`. The included `input` is based on upstream `tests/regression/d3q19-io/iodrop-mpi1-io1.inp`, with the processor grid changed from 4×1×1 to 1×1×1 for a serial run, `config_at_end yes`, and `default_io_format ascii` added for inspecting results. It uses upstream coefficients rather than measured oil/water properties and periodic 3D boundaries rather than a confined projector plate.

For example, outside the TSS checkout:

```sh
git clone https://github.com/ludwig-cf/ludwig.git
cd ludwig
git checkout 78d83c5151ff1d4c4201a96b266a08e97047cae6
cp config/unix-gcc-default.mk config.mk
make serial
make -j4
make unit
mkdir -p run/drop
cp /absolute/path/to/tss/experiments/liquid-fluid/ludwig-reference/input run/drop/input
cd run/drop
../../src/Ludwig.exe > run.log 2>&1
```

On the tested Intel Mac the serial MPI tests and Ludwig unit suite passed. The 32³ droplet run finished normally after 20 steps, reporting phi total `2.8267019e+04` at steps 0 and 20 to its displayed precision and free-energy density from `-1.4585951837e-03` to `-1.4614771181e-03`. The version with ASCII checkpoint output took 3.15 seconds wall time. This short run does not measure exact mass conservation or validate phase physics, real material parameters, physical output time, optical appearance, GPU execution, or live/4K performance. It produced scalar `phi` and velocity checkpoints with JSON metadata; it did not produce a rendered image.

Ludwig documents OpenMP, CUDA, and HIP compute backends. A Metal backend has not been established. Further work must prioritize a tested GPU implementation on each selected platform, runtime GPU/feature discovery and benchmarking, and appropriate phase/material, boundary and light transport models.
