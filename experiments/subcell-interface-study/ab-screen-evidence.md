# Two-interface screen comparison

These are actual CPU projections of the two separately exported NVIDIA ray buffers in the [GPU run record](../gpu-job-control/subcell-ab-evidence.md). Both use the same fluid densities, dyes, optical parameters, frame, source-ray sampling, screen size and projection distance. The comparison changes the distance fields: the original uses binary-mask Euclidean distance and one-cell Gaussian smoothing, while the subcell version uses a contour interpolated from `rho - phi` without that smoothing. It is therefore **not** an isolated measurement of interpolation alone.

[Original still](output/ab-screen-20261119/original/screen-clipped.png) and [subcell still](output/ab-screen-20261119/subcell/screen-clipped.png) are each 256 × 256 pixels, with no upscaling, invented texture or noise. They use the existing collimated-ray forward projection, periodic screen and a display conversion that clips linear values outside 0–1 before sRGB encoding. Their [unclipped original](output/ab-screen-20261119/original/irradiance-linear.npy) and [unclipped subcell](output/ab-screen-20261119/subcell/irradiance-linear.npy) contain the relative linear RGB detector values. This is a single frame, not a video, a physical light measurement, or a calibrated lens model. Both images remain visibly flat and cartoon-like, with the subcell result showing more small bright speckles and changed boundary highlights; neither establishes the requested projector appearance.

| Measurement | Result |
| --- | --- |
| Linear RGB mean absolute difference across pixels and channels | 0.02199835 |
| Linear RGB root-mean-square difference | 0.05341477 |
| Linear RGB 95th percentile absolute channel difference | 0.11875410 |
| Pixels with at least one changed 8-bit encoded channel | 39.0701% |
| Pixels with any channel above linear display white, original / subcell | 2.3499% / 3.0884% |
| Maximum per-channel relative energy-accounting residual, original / subcell | 9.01e-15 / 5.26e-15 |

The per-channel energy check compares the detector sum adjusted for its sampling ratio against the transmitted ray-power sum. It checks the projection's bookkeeping, not the correctness of the upstream rays, lens or physical illumination. Changes in these metrics do not establish that either reconstruction is more accurate. Neither image has an all-zero linear RGB detector pixel in this run, but that does not establish sampling convergence. The point-spread function, finite aperture, lens aberrations and true three-dimensional interface remain unmodeled.

## Reproduce the CPU projection

From the TSS root, with Python 3.12.12, NumPy 2.4.4 and Pillow 12.2.0, select fresh output directories:

```sh
mise x python@3.12.12 -- python experiments/optics-next/screen_projection.py experiments/gpu-job-control/output/subcell-ab-validation-20261119/original-rays.f32le experiments/subcell-interface-study/output/ab-screen-20261119/original --grid 512 --detector-grid 256 --distance 0.25
mise x python@3.12.12 -- python experiments/optics-next/screen_projection.py experiments/gpu-job-control/output/subcell-ab-validation-20261119/subcell-rays.f32le experiments/subcell-interface-study/output/ab-screen-20261119/subcell --grid 512 --detector-grid 256 --distance 0.25
```

The existing directories must be renamed or fresh destination paths used; the projection refuses to overwrite them. `output/ab-screen-20261119/metrics.json` records exact input, script and output SHA-256 hashes and the full numerical measurements. The input ray buffers contain 512 × 512 × 8 little-endian float32 values each; every record was checked finite with its sentinel set. The optical runs and their captured telemetry are described in the linked GPU run record. Both input buffers and this study's output directory are ignored research artifacts, not a complete distributable reproduction bundle. No GPU was dispatched in this projection or image analysis.
