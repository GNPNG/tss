# Rear-slide footprint and transmission under repeated turns

At four turns of the Windows benchmark gesture, the accumulated displacement map sampled up to 10.19 source texels per projected pixel in its most compressed direction. A single distance lookup classified hundreds of partially covered projected pixels as entirely clear or filled. Replaying the same gesture with the bounded material-coordinate tug reduced maximum compression in this region to 0.86 source texels per projected pixel.

| Frozen-state measurement | Accumulated map | Bounded map |
| --- | ---: | ---: |
| Local compression, p95 / maximum, source texels per 640-pixel preview sample | 8.722 / 10.194 | 0.851 / 0.859 |
| Level-zero signed-distance reconstruction, falsely clear / filled pixels | 439 / 288 | 53 / 0 |
| RMS error in linear green transmittance | 0.043734 | 0.011228 |
| Adaptive 4 × 4 reference estimator, falsely clear / filled | 1 / 0 | 53 / 0 |
| Adaptive estimator, RMS error | 0.003247 | 0.011228 |
| RMS after the same normalized defocus pupil, level-zero / adaptive | 0.006790 / 0.000230 | 0.000651 / 0.000651 |

## Measurement

Use the bundled rear dye field at 1.25 seconds, state 30. Hold the underlying liquid fixed. The motion input follows the Windows benchmark ellipse at 40 updates per second with coordinates quantized to its 2571 × 1444 surface. The two saved 320 × 180 interleaved little-endian float32 XY displacement maps were read back from the original and corrected gesture paths after 428 updates. Both are expressed over the full gate with row zero at the top. The compact source fixture is the unmodified 512 × 288 rear byte plane from state 30.

The measurement region comprises the 25,520 projected pixels in the normalized elliptical annulus from 0.7 to 1.3 times the gesture radius, at 640 × 360. Each numerical reference ray bilinearly reconstructs original coordinates through the measured map, samples the source distance in design pixels, converts it to rear dye coverage with the shader's 1.46-pixel meniscus scale, and applies Beer–Lambert transmission at a fixed unit path and green extinction of 1.53. The reference integrates 16 × 16 positions per output pixel. The experiment isolates one absorber. Surface highlights, depth variation and the other slide are disabled for this measurement.

The level-zero comparator samples through the same warp at 512 × 288, stores encoded signed distance at half precision, then bilinearly reconstructs that distance at the preview sample. “Falsely clear” means less than 1% of full-film attenuation where the reference gives more than 10%. “Falsely filled” uses the complementary thresholds. The adaptive numerical comparator averages 4 × 4 transmitted-light samples where the largest singular value of the screen-to-source Jacobian exceeds 1.2 and the pixel can intersect the transition. It selected 6,965 annular pixels on the accumulated map and zero on the bounded map. A normalized disk of radius 6.6635 preview pixels simulates the shared defocus footprint for both estimates.

For the accumulated map at updates 2079 through 2082, the RMS error in adjacent-frame transmittance changes was 0.000693 for the level-zero comparator and 0.000573 for adaptive sampling. The peak errors were 0.0266 and 0.0219. These four frozen-source samples establish a spatial aliasing defect. They do not independently reproduce a large temporal flash. The motion path has a separate area/fold diagnostic.

## Native optical path

![The same pink rear ribbon, sharp on the left and broadened on the right](ribbon-focus.png)

Identical 48 × 48 crops from the native CPU Vulkan proof, enlarged 5× by nearest-neighbor sampling. The left image focuses the rear dye. The right focuses the clear front slide.

The opt-in live renderer uses 24 deterministic, paired pupil rays. At distorted or rotated interfaces it distributes them across a centered 6 × 4 screen-pixel footprint. Each ray samples the independently posed rear and front fields and then multiplies their transmissions. The geometric fields remain available for locating the menisci. A per-frame GPU prepass caches the two slides' normal, resolved rim support and gap depth. The front refraction and rear absorption each sample those properties at the ray's material intersection. Each uniform film retains its tint through focus. The projected source and intermediate fields use the same finite-domain sampling and crop transform. [Native footprint sampling](../../../../live-wheel/src/footprint.glsl) · [Material prepass](../../../../live-wheel/src/film-material.glsl) · [Paired optical rays](../../optics.hook)

The CPU Vulkan regression uses an isolated 3.2-source-texel-wide rear ribbon, a clear front slide, unit material thickness, a 0.41 radian rear rotation, and 160 × 90 projected pixels. It decodes the displayed green channel to linear radiance, normalizes by a separately rendered clear field and integrates positive attenuation over a fixed crop. The near-focus sum was 35.28299. The front-focus sum was 35.27943, a −0.0101% change. The peak attenuation fell from 0.77926 to 0.41863 and the measured transverse RMS width increased from 2.51675 to 2.63869 preview pixels. Clear and fully filled controls matched exactly through focus in their tested interiors. The regression tolerance on integrated attenuation is 1% to accommodate display quantization. This verifies a projected-light invariant for an isolated absorber. A physical phase-volume measurement must use the underlying world-space liquid state. [Native regression](../../../../live-wheel/src/footprint_test.rs)

The footprint method follows the image-space derivative treatment of anisotropic texture filtering. Its adaptive numerical comparator integrates the nonlinear transmission before averaging. The live implementation reuses the fixed pupil sample budget. [PBRT image-texture filtering](https://www.pbr-book.org/4ed/Textures_and_Materials/Image_Texture)

## Repeat

From this directory, use the C++ compiler, zlib and Imath.

```sh
c++ -std=c++17 -O3 sampling-study.cpp -lz $(pkg-config --cflags --libs Imath) -o sampling-study
./sampling-study rear-030.r8.gz accumulated-0428.rgf32.gz
./sampling-study rear-030.r8.gz bounded-0428.rgf32.gz
./sampling-study rear-030.r8.gz -
```

An optional final prefix writes the three 640 × 360 candidate and reference float planes. Pixels outside the measurement crop are zero. The native transmission regression runs from the repository root with the software Vulkan ICD explicitly selected.

```sh
cargo test --locked --manifest-path live-wheel/Cargo.toml --features wgpu/vulkan-portability \
  --lib projected_ribbon_retains_tint_through_focus -- --ignored --nocapture
```
