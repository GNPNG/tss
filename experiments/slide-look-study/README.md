# Violet and cyan slide study

This standalone renderer follows two dyed-liquid layers from their starting geometry to a transmitted-light image. Both the composite and solo views use the same camera and lamp. The study uses a 16:9 crop of the projected field.

1. Create the two initial masks with the [geometry exporter](geometry.md).
2. Advance the two layers with [Basilisk](solver.md). Each frame carries the phase fraction reconstructed from that layer's own simulation.
3. Use CImg's Euclidean distance transform to prepare the two data channels. The shader reads R as the violet layer and G as the cyan layer. Zero selects a clear slide. Distances use 1280 × 720 reference coordinates, so the optical widths are independent of output resolution.
4. Render with Windows FFmpeg, Vulkan and libplacebo. The projector hook samples both slides through a common lens aperture and multiplies their RGB transmittances in linear light. The broad variation in optical thickness, pupil, concentration and focus are appearance parameters.

To build the field packer, supply the include directory containing CImg.h from [CImg v3.6.0](https://github.com/GreycLab/CImg/tree/01230c15cfe23d5ffd9b1d1ba05aefdbcac6ab00). The executable reads binary PGM files and writes big-endian 16-bit RGB PPM data.

```sh
c++ -O2 -std=c++17 -I "$CIMG_INCLUDE_DIR" pack-fields.cpp -o pack-fields
./pack-sequence.sh ./pack-fields solver-out 96 performance.mkv
./pack-fields solver-out/violet/048.pgm solver-out/cyan/048.pgm 1280 composite frame.ppm
```

For a solo plate select `violet` or `cyan` in place of `composite`. To reproduce the stills, pack the three views at frame 048 into one lossless FFV1 RGB sequence. The three images are in composite, violet, cyan order. Use 1 fps for the three-image sequence and 24 fps for the motion sequence.

On Windows place the input and `optics.hook` alongside `render-plate.ps1`. Pass the native FFmpeg executable path, the built [Windows GPU job guard](../gpu-job-control-rust/README.md), its shared reservation path, and the target GPU model substring and UUID. These are explicit so that both the Vulkan device and the monitored device can be verified on a multi-adapter machine.

```powershell
.\render-plate.ps1 -InputFile performance.mkv -OutputFile slides.mp4 -Width 3840 -FFmpeg $ffmpegExe -JobGuard $guardExe -Reservation $reservationPath -Adapter $gpuName -NvidiaUuid $uuid
.\render-plate.ps1 -InputFile views.mkv -OutputFile 'view-%02d.png' -Width 3840 -FFmpeg $ffmpegExe -JobGuard $guardExe -Reservation $reservationPath -Adapter $gpuName -NvidiaUuid $uuid
```

The glass gaps are fixed throughout these four seconds. The initial swirls differ across the two slides. Microscale drainage at droplet contact, pressure-driven changes to the gap and music or gesture control are outside this visual study.

For the independent still views at two seconds, these commands pack the three exposures using FFmpeg's lossless FFV1 encoder.

```sh
for view in composite violet cyan; do
    ./pack-fields solver-out/violet/048.pgm solver-out/cyan/048.pgm 1280 "$view" "$view.ppm"
done
cat composite.ppm violet.ppm cyan.ppm | ffmpeg -f image2pipe -c:v ppm -framerate 1 -i pipe:0 -pix_fmt gbrp16le -c:v ffv1 -threads 2 -level 3 -g 1 -color_range pc -colorspace rgb -color_primaries bt709 -color_trc iec61966-2-1 views.mkv
```

FFmpeg exports the GPU RGB proof exposures as full-range 16-bit PNGs with an sRGB transfer function and BT.709 primaries. Convert the composite to the Golden Nozzle 16-bit Rec.709 digital plate using a transfer conversion with FFmpeg's zscale:

```sh
ffmpeg -i view-01.png -vf 'format=gbrp16le,zscale=pin=709:tin=iec61966-2-1:min=gbr:rin=full:p=709:t=709:m=gbr:r=full,format=gbrp16le' -pix_fmt rgb48be -color_range pc -colorspace rgb -color_primaries bt709 -color_trc bt709 -frames:v 1 -threads 2 violet-cyan-rec709.png
```

## Wheel colours

Pass `-Wheel blue-green`, `blue-red`, `blue-yellow`, `violet-blue`, `violet-green`, `violet-red` or `yellow-red` to either Windows render command above. An omitted wheel uses the approved Violet / Blue optics. The wheel can change between renders of the same lossless field sequence.

The [wheel registry](../../WebAssets/assets/mathmos-wheels.json) supplies the controller's representative colours and the optical fits for the six additional projections. The script reads the registry at its repository path. For a standalone render copy it beside the script and shader or pass `-PresetFile` with its path. Each reference ID identifies the corresponding owner-supplied projection photograph.

The renderer converts the carrier, rear-slide, front-slide and overlap samples from display sRGB into linear light. Each solo exposure determines an effective optical density relative to the carrier. The shared segment of a ray through both dyes takes the measured overlap density, bounded below by each solo density per channel. The rest of the ray traverses the corresponding single dye. A measured boundary hue may modify the rear meniscus over a few design pixels. These RGB fits describe the appearance of the supplied photographs. Their chemistry and absorption spectra have not been measured.

The [four-second render](approval/violet-cyan-4k.mp4) decoded as 96 distinct 3840 × 2160 frames at 24 fps, 8-bit H.264 with BT.709 signaling. FFprobe identifies the [full-resolution still](approval/violet-cyan-rec709.png) as 16-bit RGB with BT.709 primaries and transfer. The [violet exposure](approval/violet-slide.png) and [cyan exposure](approval/cyan-slide.png) each show the corresponding slide at frame 048. Windows FFmpeg selected the GTX 1650 Ti Max-Q Vulkan device for both runs. The standalone study awaits visual approval.
