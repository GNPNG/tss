# Filament optical-layer diagnostic

This is a static **optics experiment**, not a liquid simulation, TSS integration, approved reference match, or production renderer. It uses original generated geometric test fixtures: a checkerboard behind two closed rectangular slabs. The slabs are independently enabled cyan-front and magenta-rear test materials using glTF transmission, volume attenuation, and index-of-refraction extensions. They are not intended to look like oil. No third-party artwork is included.

The experiment checks whether a particular renderer configuration allows the rear slab to affect the pixels seen through the front slab. It also checks whether changing the viewer's depth-of-field focus setting changes the rendered image. The output is 8-bit PPM diagnostic imagery; this is not a 16-bit master, 4K export, frame-rate test, or validated projector or microscope optical model.

## Reproduce

Requires Node.js and the official Filament v1.50.1 macOS x86_64 `gltf_viewer` executable. The tested archive is [`filament-v1.50.1-mac.tgz`](https://github.com/google/filament/releases/download/v1.50.1/filament-v1.50.1-mac.tgz); the observed archive SHA-256 was `5545ab5a6cd92c6aeee992cef291540225a734a80a2e1cb68b3d2124e3d94fb9`. This is an observed checksum, not an independently authenticated signature. See [the initial Filament Radeon evaluation](../../docs/filament-radeon-evaluation.md) for acquisition instructions and version limits.

From the TSS root, substituting the absolute path to the extracted executable:

```sh
node experiments/filament-optics/run.mjs /path/to/filament/bin/gltf_viewer metal
```

The runner creates a fresh temporary directory, generates its own glTF test scenes, launches the official viewer headlessly in default and `-u` ubershader modes, captures logs and images, and writes `results.json`. It prints the temporary directory and all calculated metrics. The tool also accepts `vulkan` or `opengl` as the second argument, but neither has been tested for this scene. `generate.mjs` can be run independently with a destination directory to inspect the glTF scenes.

Four default-material scenes are rendered: clear checkerboard, front slab only, rear slab only, and both slabs. Front/rear/both are also rendered with the viewer's supported ubershader option. The slabs have identical thickness `0.1`, `ior=1.4`, full transmission, and different attenuation colors. A central region inside their overlap is compared byte-for-byte in the PPM outputs. A second comparison changes viewer `cameraFocusDistance` from `0.2` to `10`, with depth of field enabled and aperture set to `1`; it measures output image differences, not optical accuracy. Pixel coordinates refer to the viewer output, not a physical screen calibration.

## Observed result

On 2026-11-19, Filament v1.50.1 reported Metal on the AMD Radeon Pro 5300M. For the 614 × 640 capture, the overlap region is `x=[270,340)`, `y=[280,350)`.

| Comparison | Matching pixels | Total pixels | Maximum 8-bit channel difference |
| --- | ---: | ---: | ---: |
| Default material: front only vs both slabs | 4,900 | 4,900 | 0 |
| Ubershader: front only vs both slabs | 4,900 | 4,900 | 0 |
| Default material: clear vs rear only | 0 | 4,900 | 157 |

In this specific scene and settings, the rear slab changes the checkerboard on its own, but did not change the central pixels when the front slab was also present. This does not prove that every Filament material, rendering configuration or scene behaves identically. It does demonstrate that the tested default and ubershader glTF paths cannot be assumed to preserve the overlapping liquid-layer contribution TSS requires.

The two focus captures differed in 213,504 color channels, with an average absolute difference of about 6.78/255 across the image. The viewer output settings confirm both captures had depth of field enabled and the requested distinct focus distances. That confirms the setting affects this capture; it does not validate per-layer projector focus, correct occlusion ordering, or performance savings.

Filament's [material documentation](https://google.github.io/filament/Materials.md.html) describes cubemap refraction as refracting the environment only and screen-space refraction as using opaque scene objects with simplified exiting rays. These documented approximations and the measured result require investigation of a supported multi-layer optical composition strategy before adopting this engine for TSS. In particular, a single flattened blur and ordinary alpha-over are not established as equivalent to independently refracted, transmitting layers at different focal depths. The fluid model and the high-resolution/offline path remain separate work.
