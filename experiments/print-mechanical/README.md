The native fixture captures a coated LP gatefold and an uncoated 7-inch label from the selected wheel. It supplies separate registered 32 × 24 pixel marks for process-rich-black and actual gold/foil separations, plus the EPNI-006 colophon. The verifier reads every TIFF pixel with ImageMagick, compares the full 16-bit press PDF image against it, checks both embedded copies of the original ECI stock ICC, and runs the PDF through Ghostscript's separating device.

Set `TSS_ECI_PROFILE_DIR` to the directory containing the two original ECI profiles. Set `TSS_PLATE_CAPTURE_DIR` to an empty directory. Build the Rust live-wheel tests and run `coral_capture::capture_print_mechanicals --ignored --exact --nocapture` on the chosen discrete adapter through its GPU reservation guard. Then run the CPU checks.

```sh
node experiments/print-mechanical/verify.mjs "$TSS_PLATE_CAPTURE_DIR" "$TSS_ECI_PROFILE_DIR"
```

The raster comparison uses a Q16 or higher ImageMagick build. The independent inspections use ExifTool, qpdf and Ghostscript.

Argyll emitted a gamut-mapper non-monotonicity warning while calculating the uncoated perceptual link. The independent 1,025-step neutral-axis lookup found zero reversals in output L* for both stocks. That measurement covers the neutral axis. The files remain digital separations, and the target press, stock and foil require physical proof for final appearance. Run `node experiments/print-mechanical/verify-neutral.mjs /path/to/xicclu "$TSS_ECI_PROFILE_DIR"` to repeat the measurement.
