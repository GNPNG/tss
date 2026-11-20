# Violet/Blue focus sweep

[Five-second clip](violet-blue-rack-focus.mp4) · [Endpoint comparison](violet-blue-focus-comparison.png) · [Fixed 16-bit input](field-048.png)

The input is frame 048 from the two Basilisk slide simulations. The red channel encodes the rear slide's signed distance and the green channel encodes the front slide's. Both fields, the lamp, the pupil and the projector position stay fixed. The shared optics shader sweeps the conjugate object distance from 53.4 mm at the rear slide to 51.8 mm at the front. These dimensions describe the virtual projector used for this proof.

The 768 × 432 render used libplacebo 6.338.2 on a SwiftShader Vulkan CPU device. The H.264 clip contains 120 frames at a playback rate of 24 fps. The endpoints hold for 21 frames each. The render has 80 distinct frames.

At pixels within 1.5 design pixels of one slide's boundary and at least 18 design pixels from the other boundary, the following mean RGB gradient magnitudes were measured in the uncompressed output. Magnitudes use centered horizontal and vertical differences per output pixel, with each sRGB channel on a 0–255 scale.

| Slide boundary | Pixels | Rear focus | Front focus |
| --- | ---: | ---: | ---: |
| Rear | 4,754 | 45.12 | 7.70 |
| Front | 4,298 | 9.88 | 69.76 |

## Reproduce the frames

Run these commands in a scratch directory on a little-endian machine. Set `study` to the directory containing `optics.hook`. The native runner takes libplacebo development headers and library, Vulkan headers, and the SwiftShader Vulkan driver. Set `VK_DRIVER_FILES` to its ICD manifest. Older loaders use `VK_ICD_FILENAMES`. The runner requests SwiftShader by device name and checks the reported Vulkan device type before rendering.

```sh
cc -std=c11 -O2 "$study/focus-proof.c" $(pkg-config --cflags --libs libplacebo) -o focus-proof
ffmpeg -i "$study/approval/focus/field-048.png" -vf scale=768:432:flags=bicubic \
    -f rawvideo -pix_fmt rgba64le field.rgba16
sed 's@^// TSS_WHEEL_MATERIAL$@#define TSS_FOCUS_PLANE (smoothstep(21.0, 100.0, float(frame)))@' \
    "$study/optics.hook" > sweep.hook
./focus-proof 768 432 field.rgba16 sweep.hook 120 focus.rgba
ffmpeg -f rawvideo -framerate 24 -pixel_format rgba -video_size 768x432 -i focus.rgba \
    -vf 'scale=out_color_matrix=bt709:out_range=full,format=yuv444p12le,colorspace=iall=bt709:itrc=srgb:irange=pc:all=bt709:range=tv:format=yuv420p' \
    -c:v libx264 -crf 15 -preset medium -colorspace bt709 -color_trc bt709 \
    -color_primaries bt709 -color_range tv -movflags +faststart rack-focus.mp4
```

The input PNG has SHA-256 `eb2d893e352b8d14091fac15f697cf2976c4c3a522e138a4a962a42d2f68ce16`. Each raw frame is 1,327,104 bytes. The comparison uses the first and last frames before video encoding.
