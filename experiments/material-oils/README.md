# PDMS/water slide motion

The two native wheel caches contain four independent finite-volume trajectories. Each measured PDMS/water grade has the original violet rear mask and vortex in its red channel and the original cyan front mask and vortex in its green channel. Switching grade preserves the geometry, initial velocity field, gap and projection window of that slide.

The calculation uses Basilisk's two-phase incompressible Navier–Stokes solver, geometric VOF advection, implicit viscosity and curvature force. The existing confined-slide solver adds the fixed-gap acceleration `−12 μu/(ρh²)`. The 50 mm-wide gate sits inside a closed 68.75 mm square domain with a 2.5 mm plate gap. These are selected scene dimensions. The 50 mm/s nondimensionalization speed sets physical time. Evaluating the existing initial stream function on the level-7 cell centres gives peak in-plane speeds of 4.88 mm/s on the front and 5.70 mm/s on the rear. The initial velocity field is specified at time zero and then evolves freely. [Project solver and SI scaling](../slide-look-study/solver.c) · [Basilisk momentum solver](https://basilisk.fr/src/navier-stokes/centered.h) · [Surface tension and timestep bound](https://basilisk.fr/src/tension.h)

| Fluid | Density (kg/m³) | Dynamic viscosity (Pa·s) | Oil–water interfacial tension (mN/m) |
| --- | ---: | ---: | ---: |
| PDMS 20 cSt | 950 | 0.019 | 40.9 ± 0.5 |
| PDMS 1000 cSt | 970 | 0.970 | 40.8 ± 1.0 |
| DI water | 998 | 0.00089 | — |

The nominal oil grades refer to 25 °C and 1 atm. The paper used pendant-drop tensiometry and gives no separate bath temperature for that measurement. The two principal simulations take all three properties from the relevant measured grade. [Ji, Yang & Feng, Table 1 and Methods](https://pmc.ncbi.nlm.nih.gov/articles/PMC8563946/) · [Project material registry](../../live-wheel/materials/fluids-and-glass.json)

The native assets contain 96 recorded states at 24 Hz. The first and last states span 95/24 s. For this geometry the physical time in seconds equals solver time multiplied by 1.375. Each adaptive quadtree has levels 5–7 and a minimum cell width of 0.5371 mm. The solver reconstructs the interface onto a 1024 × 1024 VOF coverage field. The existing CImg optical-distance packer writes the 704 × 704 square field, the centred 512 × 288 gate and the two signed-distance channels in the established `TSSDF002` format. [Geometry source](../slide-look-study/geometry.hpp) · [Cache packer](../slide-look-study/pack-world-cache.cpp)

The frame log records phase volume as an integral over the full unit square, proportional to the amount of oil at constant gap. Gate occupancy is classified at 50% VOF. Contour distances use the Euclidean distance from each rasterized interface to the nearest opposing interface in the whole world, sampled in both directions inside the gate. The distances measure shape separation. Thin films between merging drops, contact-angle hysteresis, through-gap meniscus shape and measured real-world trajectories remain outside the fixed-gap model.

At one physical second, the two grades occupied different phases on 18.49% of the front gate and 26.11% of the rear gate. The mean separation of their contours was 2.07 mm on the front and 1.95 mm on the rear. All four initial and final VOF fields stayed in [0, 1], as did every intermediate recorded state. The largest disagreement between a packed contour and its original VOF phase classification was 0.295% of the gate across all 384 fields. Recorded timestamps agreed with the intended physical cadence to within 0.07 µs at the log's printed precision. [Per-frame measurements](frames.csv) · [Contour comparisons and cache hashes](verified.json)

| Slide and grade | Numerical steps | Largest relative volume drift | Gate area that changed phase from the initial state at 1 s |
| --- | ---: | ---: | ---: |
| Front, 20 cSt | 3,718 | 0.0582% | 20.34% |
| Front, 1000 cSt | 15,216 | 0.0189% | 8.62% |
| Rear, 20 cSt | 3,716 | 0.0212% | 29.38% |
| Rear, 1000 cSt | 15,216 | 0.0320% | 9.23% |

The 20 cSt trajectories ran on Intel macOS. The 1000 cSt trajectories ran under Ubuntu WSL with GCC 15.2.0. Both used solver source SHA-256 `5d9543c9d06ba992fa7c24c6b18e8d89dbd810e72a46ae5d5828fff44ab5fa64`. An overlapping high-grade front run on the Mac reproduced the WSL initial field byte for byte. At 1 s the two hosts differed on 0.274% of the gate and their mean contour distance was 0.031 mm. At 1/6 s, raising the low-grade front grid ceiling from level 7 to 8 changed gate occupancy on 3.04% of its area. The two level-7 grades differed on 12.50% of that gate at the same time. This short refinement check measures early sensitivity only. [Cross-host measurements](platform-prefix.json) · [Early refinement measurements](mesh-early.json)

To reproduce the CPU results, build the existing solver using the [pinned Basilisk toolchain](../basilisk-study/README.md) and the [slide solver build command](../slide-look-study/solver.md). Install CImg 3.6.1, a C++17 compiler, zlib, Node, librsvg and ImageMagick. The recorded rasterizer was librsvg 2.62.3 with Cairo 1.18.6 and ImageMagick 7.1.2-30 Q16-HDRI. Run with a new output directory.

```sh
SOLVER=/absolute/path/to/solver CIMG_INCLUDE=/path/to/CImg ./experiments/material-oils/run.sh /path/to/new-run
```

The runner generates both initial masks and vortex trials, packed native assets, per-frame measurements and gate comparisons. The original 1024-square front mask has SHA-256 `07c95a0b5874465233127254efffd549ae12485dd8213f318bc8289a3a13b94e` and the rear mask `c7e622aed3cb7acff1557d576986ac631836f749a576786205b50f20a1cb95e4`. Source and packed fields use the first image row at the top.
