# Two front oils on one Violet / Blue wheel

Open either project in the desktop editor. Both use the same original rear wheel, front-slide focus of 0.90 and 640 × 360 live optics. Change **Front film oil** to switch grades. **Projected films → Rear only** isolates the violet rear wheel. [20 cSt project](front-20.tss) · [1,000 cSt project](front-1000.tss)

The four-second comparison shows both front grades at the same 96 times on the Radeon Pro 5300M using Metal. The first frame has identical oil contours. At two seconds, switching the front grade changed 73,668 of 147,456 sampled front-field values and zero rear values. In the reciprocal test, switching the rear grade changed 96,145 rear values and zero front values. The preview and native movie outputs matched byte for byte at this timestamp for both front oils. [Movie](radeon/front-oils.mp4) · [Still at two seconds](radeon/at-two-seconds.png) · [Capture data](radeon/capture.json)

The two grades use separately measured viscosity, density and oil–water interfacial tension. Their motion is precomputed in the fixed slide gap. The desktop GPU interpolates the fields and renders the dyed films, meniscus and optical focus. The movie plays at 24 fps. This capture does not measure interactive frame rate. [Solver, material properties and grid checks](../material-oils/README.md)

To reproduce, reserve one hardware GPU and run the ignored Rust test `coral_capture::oil_comparison::compare_projected_oils` in the `live-wheel` crate. Set `TSS_LIVE_ADAPTER` to the device name and `TSS_OIL_CAPTURE_DIR` to a fresh directory path. Run `experiments/oil-projection/encode.sh <capture-directory> <new-review-directory>` for the captioned movie and still.
