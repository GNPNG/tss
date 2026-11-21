---
name: golden-nozzle-media-pipeline
description: Prepress CMYK print mechanical specifications and 4K UHD digital/video asset standards for The Golden Nozzle releases on The Elvis P. Nixon Institute (EPNI).
---

# Golden Nozzle Media & Prepress Pipeline

## 1. Dual-Deliverable Mandate

Every visual asset must target one of two explicit output profiles:
1. **Orthographic Print Mechanical (`PRINT_CMYK`)**: Flat, front-facing, borderless full-bleed artwork with zero physical props, zero simulated ring-wear, and zero perspective distortion.
2. **4K Digital & Motion Master (`DIGITAL_4K`)**: Display-referred `sRGB` / `Rec.709` or `Display P3` high-bit-depth raster/vector plates formatted for streaming DSPs, 16:9 promotional video visualizers, and 9:16 vertical social campaigns.

## 2. Print Prepress Specifications (`PRINT_CMYK`)

| Format | Trim Size (mm) | Bleed (mm) | Canvas w/ Bleed (mm) | Min Pixel Dimensions @ 300 DPI | Safe Type Margin (mm) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **12" LP Single Jacket** | `314 x 314` | `3.0` all sides | `320 x 320` | `3780 x 3780 px` | `6.0` inside trim |
| **12" LP Gatefold Spread** | `634 x 314` (incl. `6 mm` spine) | `3.0` all sides | `640 x 320` | `7559 x 3780 px` | `6.0` inside trim / spine |
| **7" 45rpm Picture Sleeve** | `184 x 184` | `3.0` all sides | `190 x 190` | `2244 x 2244 px` | `5.0` inside trim |
| **7" Vinyl Center Label** | `92` dia. (`38.1` spindle hole) | `2.0` outer | `96 x 96` | `1134 x 1134 px` | `4.0` from outer/inner die |
| **12" Vinyl Center Label** | `100` dia. (`7.24` spindle hole) | `2.0` outer | `104 x 104` | `1228 x 1228 px` | `4.0` from outer/inner die |
| **Mini-LP CD Gatefold** | `275 x 135` (incl. `5 mm` spine) | `3.0` all sides | `281 x 141` | `3319 x 1665 px` | `4.5` inside trim |
| **Archival Obi Strip (12")** | `65 x 314` (front wrap) | `3.0` all sides | `71 x 320` | `839 x 3780 px` | `4.0` inside fold/trim |

### Color Separation & Ink Rules

* **ICC Profile**: `ISO Coated v2 300% (ECI)` for gloss/satin stock; `PSO Uncoated ISO12647 (ECI)` for uncoated archival/matte board (Albums I, II, III, and *The Aldebaran Tapes*).
* **Total Area Coverage (TAC)**: Maximum `300%` (coated) / `280%` (uncoated) via Medium GCR separation.
* **Rich Black (Large Solids)**: `C:60 M:40 Y:40 K:100`.
* **Fine Print & Colophon Text**: Strictly `C:0 M:0 Y:0 K:100` (100% Black overprint, minimum `6 pt` serif / `5.5 pt` sans-serif) so member credits (`Bijon Roy, Brett Whitty, Jamie Mellway`), studio (`The Whitty Compound`), and `EPNI` catalog blocks never misregister on press.
* **Spot Metallic & Foil Callouts**: Alchemical sigil and title gilding mapped to spot channel `PANTONE 871 C` (metallic muted gold) or hot-foil stamping plate (`FOIL_GOLD`).

## 3. 4K Digital & Video Specifications (`DIGITAL_4K`)

| Deliverable | Aspect Ratio | Pixel Dimensions | Color Space | Bit Depth / Format | Composition Rule |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **DSP Master Cover** | `1:1` | `4000 x 4000 px` | `sRGB IEC61966-2.1` | `16-bit PNG` / `TIFF` | Full-bleed front art; zero promo stamps on retail master |
| **4K Video / Visualizer Plate** | `16:9` | `3840 x 2160 px` | `Rec.709` / `Display P3` | `16-bit PNG` (layered BG/Mid/FG) | Horizon locked at `38%` height (`y = 1339 px` from top) for pan/parallax |
| **Vertical Reel / Promo Story** | `9:16` | `2160 x 3840 px` | `sRGB` / `Rec.709` | `16-bit PNG` | Sigil & band title in top `15%–28%` safe zone; lower `20%` clear for UI |
| **5-Gatefold Cycle Panorama** | `5:1` | `10800 x 2160 px` | `Display P3` / `sRGB` | `16-bit TIFF` / `PNG` | Seamless edge-to-edge join across `EPNI-006` through `EPNI-010` |

The confirmed horizon position is `y = 1339 px` from the top, approximately 38% from the bottom. The horizon can be optional and adjustable for abstract plates. The specification above is preserved as supplied.

## 4. Deterministic Raster/Prepress Conversion Commands (CLI Reference)

When converting generated `sRGB` masters locally via ImageMagick 7 (`magick`):
* **300 DPI CMYK Prepress TIFF (300% TAC)**:
  `magick input_master.png -profile sRGB.icc -profile ISOcoated_v2_300_eci.icc -density 300 -units PixelsPerInch -compress LZW output_cmyk_300dpi.tif`
* **4K Square DSP Master**:
  `magick input_master.png -colorspace sRGB -filter Lanczos -resize 4000x4000 -depth 16 output_dsp_4000.png`

### Native print mechanicals

In the Windows editor, select a visible wheel and open **Selected wheel · production stills**. Choose any PRINT_CMYK format and its stock. **Preview current framing** shows the current optical projection beneath the registered die, spindle, fold and safety guides. The export captures the wheel at full density in its presented pose, crops its optical gate without stretching it, and renders to the full-bleed 300 ppi dimensions above. Choose the matching original destination ICC from the [ECI Offset 2009 archive](https://eci.org/lib/exe/eci_offset_2009.zip).

The bundle contains these files.

- `artwork.tif` carries the 16-bit-per-channel CMYK process artwork, stock profile and optional C60 M40 Y40 K100 solids. Its full canvas includes bleed and the material later removed by each circular die.
- `press.pdf` contains that same 16-bit process image, the stock output intent, physical TrimBox and BleedBox, plus any selected colophon and spot finishing. The optional EPNI colophon uses positive 6.5 pt Source Sans 3 outlines in K100 overprint on an uninked panel. Source Sans 3 is distributed under the SIL Open Font License. [Typeface source](https://github.com/adobe-fonts/source-sans/tree/3.052R)
- `dieline.svg` is a separate, registered non-printing finishing guide in millimetres. The PDF TrimBox is rectangular. Use the SVG outer and spindle dies for circular labels.
- `mechanical.json` records the physical dimensions, film times, stock, black-generation rule, TAC ceiling and measured maximum across all process pixels.

To add gilding to the title or sigil, select **PANTONE 871 C** or **FOIL_GOLD** and supply the artwork as a registered, full-canvas, one-bit grayscale PNG. Marked black pixels print on the named separation and white remains clear. The screen alternate approximates the spot. The rich-black solids option accepts a separate mask in the same format. Put the desired large solid areas on that plate. Both stencils use the exact raster dimensions in the print specification. Keep finishing marks and the optional colophon clear of each other.
