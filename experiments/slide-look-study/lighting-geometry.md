# Geometry for the caustic lighting comparison

The [native exporter](export-lighting.cpp) reads [frame 048](approval/focus/field-048.png) at its original 1024 × 576 resolution. Rear and front remain separate through export. The files in [geometry](approval/caustics/geometry/) use a 160 × 90 mm crop. The in-plane scale and the 3D relief below are assumptions for the lighting experiment. The source simulation supplies the in-plane boundaries.

## Distance images

The input PNG packs unsigned 16-bit distances into red for rear and green for front. The exporter reads the integer samples without a transfer function and computes `d = 96 × (sample / 65535 − 0.5)`. Positive values indicate dye, negative values indicate clear carrier, with saturation at ±48. Both EXRs contain the signed distance in 720p design pixels, repeated in three 32-bit float channels with lossless ZIP compression. Load each as **Non-Color** data in Blender. One design pixel corresponds to 0.125 mm at the assumed scale.

The EXR data window begins at (0, 0) at the top left. World X points right and Y points up. Pixel center (x, y) maps to `X = 0.160 × ((x + 0.5)/1024 − 0.5)` and `Y = 0.090 × (0.5 − (y + 0.5)/576)` in metres. With the ordinary bottom-left texture convention the inverse UV mapping is `(X/0.160 + 0.5, Y/0.090 + 0.5)`.

## Closed PLY solids

Each slide uses its own clipped footprint on a 512 × 288 triangulated lattice. The surface linearly interpolates bilinear samples of the source distance within each triangle. Shared crossing vertices prevent cracks. At the image boundary the distance uses edge-clamped sampling and the solid closes against the crop. The rear bottom lies at Z = +2.5 mm and the front bottom at Z = 0. Dye thickness extends along +Z.

The edge thickness is 0.35 mm and the interior thickness is 1.00 mm. For a configurable transition width w (default 1.0 mm), let `q = clamp(d × 0.125 mm / w, 0, 1)`. The upper surface rises above the base by `0.35 mm + 0.65 mm × q² × (3 − 2q)`. Each flat control uses the area-weighted mean thickness of its corresponding relief mesh. Its X/Y, topology and volume therefore match across the lighting comparison.

Binary little-endian PLY vertices carry float32 X/Y/Z and normal X/Y/Z in object space. Upper normals use area-weighted geometric triangle normals, bottom normals point along −Z. At the sharp side wall use geometric face normals. The flat controls can use flat shading throughout. The new objects are already in metres, with identity transforms.

| Slide | Connected solids | Vertices | Triangles | Paired edges | Dye footprint (m²) | Volume (m³) | Flat depth (mm) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Rear | 16 | 168,070 | 336,120 | 504,180 | 0.00775511 | 0.00000752742 | 0.970640 |
| Front | 15 | 126,362 | 252,696 | 379,044 | 0.00578469 | 0.00000557615 | 0.963949 |

The exporter reopened each EXR and compared every RGB float against the source decode. An independent binary PLY reader found exactly two oppositely directed triangle edges at every mesh edge and matching XY and face-index bytes in each relief/control pair. Unit-normal length errors stayed below 4.2 × 10⁻⁸. Independent point-in-triangle checks at six PNG pixel centers per slide confirmed the world-space orientation and phase occupancy. For example, at (768,144) the rear/front distances are −8.137/+48.000 and only the front geometry covers that point. At (240,344) the rear/front distances are +47.166/−48.000 and only the rear covers it.

## Reproduce

From this directory, with libpng and OpenEXR available to pkg-config, build the C++17 exporter and choose a new output directory. The optional last argument gives the meniscus width in millimetres.

```sh
c++ -std=c++17 -O2 -Wall export-lighting.cpp $(pkg-config --cflags --libs libpng OpenEXR) -o export-lighting
./export-lighting approval/focus/field-048.png geometry-rebuild 1.0
```

This run used libpng 1.6.59 and OpenEXR 3.4.15. On the study host the installed OpenEXR library requires OpenJPH 0.31.0 and the active Homebrew OpenJPH is 0.33.0. The run loaded an isolated build of the exact required OpenJPH version. [OpenJPH 0.31.0](https://github.com/aous72/OpenJPH/tree/c68064d0e4cad8e96bab9a068f6cc4e7799744fc)
