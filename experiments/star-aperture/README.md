# Star aperture and travelling matte

Run `./prepare.sh /path/to/new-directory` from this directory with a C11 compiler, pkg-config, Cairo and FFmpeg. Each clip is 960 × 540, 72 frames at 24 FPS. All three are silent. The source is a five-point paper-cut aperture with three differently coloured bands and a moving centre. The matte is an independently moving disc with an off-centre opening.

**star-green.mp4** carries the aperture on green in lossless RGB H.264. **star-alpha.mov** carries the same artwork with straight alpha. **moving-matte.mov** supplies a third track for exercising Boolean masks. Transparent PNG samples in the two MOV files permit byte-exact comparison of the input frames. The CPU-generated clips are test media. The native editor supplies the oil, keying, compositing and projection.
