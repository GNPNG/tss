# Star aperture and travelling matte

Run `./prepare.sh /path/to/new-directory` from this directory with a C11 compiler, pkg-config, Cairo and FFmpeg. Each clip is 960 × 540, 72 frames at 24 FPS. All three are silent. The source is a five-point paper-cut aperture with three differently coloured bands and a moving centre. The matte is an independently moving disc with an off-centre opening.

**star-green.mp4** carries the aperture on green in lossless RGB H.264. **star-alpha.mov** carries the same artwork with straight alpha. **moving-matte.mov** supplies a third track for exercising Boolean masks. Transparent PNG samples in the two MOV files permit byte-exact comparison of the input frames. The CPU-generated clips are test media. The native editor supplies the oil, keying, compositing and projection.

## Open the composition as a project

Generate the two fixture clips with `prepare.sh`, then run `node project.mjs <prepared-clips-directory> <new-project-directory>`. The fixture builder uses the same portable project validator and gesture scheduler as the editor. The output folder contains **Star Aperture.tss**, the star, its matte and the two-cue song. Keep the four files together.

Open **Star Aperture.tss** in the Windows editor and press **Play**. The two gesture macros pull the rear oil at 0.25 s and rotate the front slide at 1.10 s. Select **star-green.mp4** to change its gradient or turn off **Mask A** to let the star fill the frame. Select **Violet / Blue** to move the focus between its two films.
