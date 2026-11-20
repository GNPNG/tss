# Filament Radeon Metal evaluation

On 2026-11-19, the official Google Filament **v1.50.1** macOS release's unmodified `gltf_viewer` rendered its built-in model through Metal on this Intel macOS host. The sample reported `AMD Radeon Pro 5300M` as its selected physical device and completed a single automation case with exit status zero. Its generated 614 × 640 PPM image was visually inspected and contained the rendered sample model.

This establishes that this particular prebuilt Filament sample can render on this particular GPU. It does not establish a coupled-liquid simulator, projector optics, depth-of-field accuracy, TSS integration, iPad or Windows support, live frame rate, 4K performance, or the requested visual appearance. It is a 2024 release; the Intel build and Metal compatibility of a later release remain to be established. No user system software was installed or changed.

The [v1.50.1 release](https://github.com/google/filament/releases/tag/v1.50.1) contains a macOS archive with x86_64 executables. The independently computed SHA-256 of the downloaded `filament-v1.50.1-mac.tgz` was `5545ab5a6cd92c6aeee992cef291540225a734a80a2e1cb68b3d2124e3d94fb9`; this is an observed checksum, not an independently authenticated signature. The upstream [automation schema](https://github.com/google/filament/blob/v1.50.1/libs/viewer/schemas/automation.json) allows the single-case input below.

```sh
mkdir -p /private/tmp/tss-filament-eval/v1.50.1
curl -fL --output /private/tmp/tss-filament-eval/filament-v1.50.1-mac.tgz \
  https://github.com/google/filament/releases/download/v1.50.1/filament-v1.50.1-mac.tgz
tar -xzf /private/tmp/tss-filament-eval/filament-v1.50.1-mac.tgz \
  -C /private/tmp/tss-filament-eval/v1.50.1
printf '%s\n' '[{"name":"radeon-check","base":{},"permute":{}}]' \
  > /private/tmp/tss-filament-eval/one-case.json
cd /private/tmp/tss-filament-eval/v1.50.1/filament/bin
./gltf_viewer -a metal -b /private/tmp/tss-filament-eval/one-case.json -e
```

The command produces `radeon-check0.ppm` and `radeon-check0.json` in the current directory. This is upstream's bundled demonstration model, not liquid-light imagery. Relevant output:

```text
FEngine resolved backend: Metal
Selected physical device 'AMD Radeon Pro 5300M'
Backend feature level: 3
FEngine feature level: 3
Running test 0
```

An earlier run of the default, many-case automation was interrupted after more than 180 cases; it should not be reported as a completed test suite. The single-case run above subsequently completed. The screenshot is kept only in the temporary evaluation directory; it is not included in TSS.
