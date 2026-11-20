# Pressure between two glass plates

The native preview prescribes a local change of plate separation under the performer's finger. The independent CPU reference solves for the pressure needed to produce that motion in an incompressible Newtonian film of uniform viscosity. It checks the direction and magnitude of lateral transport during compression, hold and release.

Let `s = (r/R)²`, with radius of influence `R`, unloaded plate gap `h₀`, and compression amplitude `0 ≤ a ≤ 0.65`. Within the disk, the original radial material coordinate satisfies `s₀ = s [1 − a(1−s)³]`. Differentiation gives the local gap ratio

```text
h/h₀ = ds₀/ds = 1 − a(1−s)²(1−4s).
```

At maximum compression the center reaches 35% of its unloaded gap. The displaced volume opens an annulus that reaches 116.25% at `r = R/√2`. At the outer edge the map joins the stationary material smoothly. A parcel subtending angle `dθ` occupies volume `h R² ds dθ/2 = h₀ R² ds₀ dθ/2`. Oil and carrier each obey this identity for any parcel carried by the map. Discrete sampling errors in an actual wheel require a separate atlas check.

With a fixed center and changing amplitude, continuity gives the outward depth-integrated flux `Qᵣ = h₀ (da/dt) r (1−s)³ / 2`. The sign reverses on release. The reference solves the thin-film Reynolds equation with no-slip walls using Basilisk's variable-coefficient multigrid Poisson operator. Pressure is relative to the undisturbed exterior. [Basilisk Poisson solver](https://basilisk.fr/src/poisson.h) · [Basilisk Hele–Shaw documentation](https://basilisk.fr/src/hele-shaw.h)

```text
Q = −(h³/12μ) ∇p
∇·[(h³/12μ) ∇p] = ∂h/∂t
```

The benchmark uses dimensionless coordinates, `h₀ = 1`, `12μ = 1` and `R = 0.30` in a unit square. It specifies gap displacement and solves for pressure. A force-controlled elastic glass plate, material-dependent stress relaxation, and changing interfacial tension require additional constitutive information. The reference applies to a stationary pressure center. Measured sensor force in newtons is outside this calibration.

| Grid | Relative L² flux error | Relative center pressure error |
|---|---:|---:|
| 64² | 0.3590% | 2.2131% |
| 128² | 0.0894% | 0.5544% |
| 256² | 0.0224% | 0.1386% |
| 512² | 0.0056% | 0.0346% |

Compression and release gave the same error magnitudes with opposite fluxes. Holding the gap fixed produced zero pressure difference and zero flow. [Measured results](reference.csv)

To reproduce, set `BASILISK` to the pinned compiler's source directory, add it to `PATH`, and run these commands beside a copy of `squeeze.c`. The executable exits unsuccessfully on insufficient convergence, excess flux error or a nonzero holding response. [Pinned Basilisk build](../basilisk-study/README.md)

```sh
"$BASILISK/qcc" -O2 squeeze.c -o squeeze -lm
./squeeze > reference.csv
```
