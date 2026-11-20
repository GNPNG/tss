# Coral release

In the Flower Garden Banks footage, boulder star coral sheds bundles that linger and sway in the current before they rise. The branching `Acropora` photographs show how readily individual bundles can be followed against a more intricate background. This preset borrows the staggered timing and shared drift. Each visible droplet is oil in a sealed glass slide. [NOAA spawning cruise, 1:43–2:27](https://flowergarden.noaa.gov/vid/spawningvideos.html) · [Tokai University field photographs, Amitori Bay](https://www.u-tokai.ac.jp/news-institution/1317120/)

## Simulation

The front slide begins with a bank of oil, rounded buds on necks of different widths and some detached beads. The rear slide begins with broad curling folds. The [initial geometry](geometry.cpp) sets only the first state. The existing [Basilisk solver](../slide-look-study/solver.c) advances each slide separately using geometric volume of fluid, surface tension, viscosity, pressure and plate drag. The initial in-plane velocity field strains some necks and lets others retract. A daughter takes its volume from the front bank. Each slide keeps its own dyed phase and clear carrier throughout.

Both runs use the complete closed square, the original wheel solver's dimensionless coefficients, its per-slide initial vortex, and adaptive level 9 (up to 512 cells across). The [existing CImg packer](../slide-look-study/pack-world-cache.cpp) derives signed optical distances from each reconstructed coverage frame. The native cache contains 96 forward states at 24 Hz, or 3.958 s between first and last state. The 512 × 288 gate samples a 704 × 704 world. Rear is the red data channel and front is green. The channels encode geometry independently of the chosen wheel palette.

Manual tug, slide rotation, pressure and focus operate on the cached films in the native renderer. The severing time is set by the precomputed trajectory. The soft color on the rear slide comes from projected dyed oil and optical defocus. This proof uses a confined two-phase liquid model and provides no model of gamete biology or diffusion of sperm in open water.

## Measured release

The [connectivity test](verify.cpp) thresholds the full world at 50% coverage on the source grid or at the zero signed-distance code in the optical cache. For each potential daughter it requires more than 0.01% of the world area and at least 55% overlap with the preceding bank. It follows the daughter for six additional frames by maximum overlap, excluding candidates that reconnect to the bank. The test requires separate sites at least 8% of the normalized gate apart and at least three frames between releases. Gate positions below are measured from the upper left.

| Daughter | 1024² source mask | 704² optical cache | Optical centroid (x, y) | Fraction of whole world |
| --- | --- | --- | --- | --- |
| First | Frame 65, 2.708 s | Frame 63, 2.625 s | (0.458, 0.568) | 0.235% |
| Second | Frame 74, 3.083 s | Frame 75, 3.125 s | (0.330, 0.581) | 0.212% |

At the second site the thresholded neck briefly disconnects at frame 71 then reconnects. It is counted once, at its sustained separation. The source/optical timing differences show the boundary resolution of the raster and distance reconstruction. The recorded times are specific to this grid and rendering path.

Two already-detached front beads show the shared current over the first half-second. In the 1024² source mask their centroids start at (674.5, 409.4) and (679.5, 438.5). At frame 12 they reach (689.2, 424.2) and (696.0, 455.2). Both drift down and right, with slightly different displacements. Coordinates are in pixels from the upper left, measured on the 50% phase contour.

| Slide | Initial full-domain dyed fraction | Largest relative deviation during VOF solve | Largest relative change of thresholded area in 704² cache |
| --- | --- | --- | --- |
| Rear folds | 0.124450702293 | 0.00243% | 0.0843% |
| Front bank | 0.393501031165 | 0.0000817% | 0.0405% |

With constant gap, phase area times gap gives volume. The solver integral includes material beyond the visible gate. Binary area in the optical cache is a separate measure of the encoded silhouette. [Rear numerical samples](rear-phase.csv) · [Front numerical samples](front-phase.csv)

## Reproduction

Use a new output directory. Set `BASILISK` to the Basilisk `src` directory containing `qcc` and `CIMG_INCLUDE` to the absolute include directory containing the project's pinned CImg 3.6.0 header. A C++17 compiler, zlib, librsvg and ImageMagick are needed when regenerating the asset. A regular editor launch reads the bundled [production cache](../../live-wheel/assets/coral-release.rg8.gz). [Basilisk configuration on the Intel Mac](../basilisk-study/README.md) · [CImg source revision](https://github.com/GreycLab/CImg/tree/01230c15cfe23d5ffd9b1d1ba05aefdbcac6ab00)

```sh
experiments/coral-release/reproduce.sh /path/to/new-coral-study
```

The script checks 96 full-domain phase integrals per slide and checks front-slide connectivity in both representations. A clean regeneration using this script on the Intel Mac CPU reproduced the shipped gzip byte for byte. Its SHA-256 is `98c1a58eade1c8a27dd4f148abc3f8b031e495f3acdd55e85f217161bdb79b61`.
