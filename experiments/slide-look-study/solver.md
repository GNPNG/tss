# Two independent slide trajectories

The phase masks are initial conditions. White is the dyed phase and black is the carrier. Each slide is advanced in a separate invocation of the existing Basilisk centered incompressible Navier–Stokes, geometric volume-of-fluid, viscosity, and surface-tension modules. Position and motion of one slide never enter the other solve. The fixed glass gap enters as a depth-averaged linear wall-drag approximation. A different initial divergence-free swirl is specified in each layer. Both then evolve from their own velocity, pressure and interface state.

The original wheel proofs use nondimensional artistic settings for geometry, viscosities, surface tension, vortices and fixed plate gaps. The solver also accepts measured SI material pairs for new trajectories. Through-gap meniscus dynamics, film drainage during coalescence and pressure-controlled gap changes are open. Tiny inclusions below the phase grid's resolution may disappear. A 4K optical plate samples the reconstructed interface at higher display resolution, while the fluid grid remains at its configured resolution.

On the Windows peer the existing WSL Basilisk source is in the adjacent `basilisk-wsl-study/vendor/basilisk` directory. Run from the `slide-look-study` directory. The input must be an 8-bit, single-channel binary PGM (P5) with a 16:9 aspect ratio, the first row at the top. Use fresh output directories. Run each command sequentially.

```sh
export BASILISK="$(cd ../basilisk-wsl-study/vendor/basilisk/src && pwd)"
"$BASILISK/qcc" -O2 -Wall -disable-dimensions solver.c -o solver -lm
mkdir -p solver-out/violet solver-out/cyan
OMP_NUM_THREADS=1 nice -n 5 timeout 600 ./solver inputs/violet.pgm solver-out/violet violet 96 9 >solver-out/violet.stdout 2>solver-out/violet.log
OMP_NUM_THREADS=1 nice -n 5 timeout 600 ./solver inputs/cyan.pgm solver-out/cyan cyan 96 9 >solver-out/cyan.stdout 2>solver-out/cyan.log
```

Each file is a 1024 × 576 PGM. The output writer calls Basilisk's interface reconstruction and subrectangle-fraction routines to antialias the material interface on the output grid. The square solver domain uses an adaptive grid with up to 512 cells across (level 9). Its middle 16:9 crop has the same orientation as the input. The region above and below that crop starts as a reflection of the input, continuing pools beyond the edge of the visible field. The closed domain has no external through-flow. Each frame log gives the frame number, simulation time, full-domain phase integral, and minimum and maximum phase occupancy. At playback rate 24 frames/s, 96 states cover four seconds and do not form a periodic loop.

## Run a slide with measured oil and water

The [material runner](solve-material.mjs) reads an oil grade and the paired water properties directly from the [material registry](../../live-wheel/materials/fluids-and-glass.json). The following example sets the scene width to 50 mm, the reference velocity to 50 mm/s and the fixed gap to 2.5 mm. These three choices define this example's geometry and time scale. The actual local velocity follows the initial stream function and the subsequent solve. Choose a new output directory for each run.

```sh
node solve-material.mjs ./solver inputs/violet.pgm violet-20cst violet pdms-water-20cst 50 50 2.5 96 9
node solve-material.mjs ./solver inputs/violet.pgm violet-1000cst violet pdms-water-1000cst 50 50 2.5 96 9
```

The final two arguments are snapshot count and maximum quadtree level. For a controlled comparison the command uses the same violet mask and starting vortex in each run. For a second optical slide, use its own mask, the `cyan` starting vortex and the required material. The native executable also accepts custom measured pairs directly through the `--si` arguments listed by its usage message.

With domain width $L$, reference speed $U$ and carrier density $\rho_c$, the solver uses $\rho_i^*=\rho_i/\rho_c$, $\mu_i^*=\mu_i/(\rho_c U L)$, $\sigma^*=\sigma/(\rho_c U^2 L)$ and $h^*=h/L$. Snapshots are spaced by $U/(24L)$ in solver time, giving 24 samples per physical second. The face acceleration contributed by the plates is $-12\mu\,\mathbf u/(\rho h^2)$. The SI path limits the explicit plate-drag timestep using the larger phase kinematic viscosity. The pressure, viscous, capillary and VOF operations use the existing Basilisk modules in both modes.

At level 7, a five-state CPU comparison with the settings above gave identical initial coverage for both grades. At 1/6 s, 69,424 of 589,824 visible coverage bytes differed between the two simulations. Both retained phase fractions in [0,1]. The largest relative deviation of the full-domain phase integral from its starting value was 3.17 × 10⁻⁶ with 20 cSt oil and 5.53 × 10⁻⁷ with 1,000 cSt oil. These are numerical checks on the material input path. Validation of real drop merger times would require filmed experiments with known fluids, substrate and gap.

## Recorded Windows run

The two single-core runs completed sequentially on the Windows test laptop in the `agent-sandbox` WSL distribution. Each advanced 578 numerical timesteps and emitted 96 phase snapshots from time 0 to 3.9583333. The violet run took 41.52 s wall time, the cyan run took 47.27 s. Both retained phase occupancy within [0,1]. Maximum measured relative deviation of the full-domain phase integral was 4.27 × 10⁻⁷ on violet and 1.04 × 10⁻⁶ on cyan. These figures refer to conservation across the whole square, including the portions outside the displayed crop. Of 589,824 displayed coverage bytes, 145,774 violet samples and 90,347 cyan samples differed between frames 0 and 95. Per-frame measurements are in [the violet log](solver-violet.csv) and [the cyan log](solver-cyan.csv).

| Input or output | SHA-256 |
|---|---|
| Initial violet PGM | `1df7728f159760c673edcc27036743d92648313dd5161417b80099a5831197b2` |
| Initial cyan PGM | `4263dba03a85f0270bf8e1d5ece4d7c81dc7685a5bd25d508b6e579c94d797d5` |
| Final violet PGM | `5f55787f39f8a46554e84f0478f3b337e0775bb5e9c6699cec9a3b1146a1268c` |
| Final cyan PGM | `90375708840b8e69cee1a7d3dba37c7f091c0a3b0adaeb7598cdc36bec766112` |
