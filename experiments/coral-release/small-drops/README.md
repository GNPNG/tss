# Front coral beads

The Wheel with coral beads preset uses a front film with numerous small oil droplets, a corrugated oil bank and ligaments of unequal lengths. Four longer buds protrude from the center of the bank in the opening state. The geometry generator defines the opening state. The existing Basilisk VOF solver advances the closed slide for 96 states at 24 Hz using the cyan film's normal initial flow and physical coefficients. [Initial geometry](geometry.cpp) · [Solver](../../slide-look-study/solver.c)

Ten detached daughters have centroids inside the projected gate and can be followed in both the source VOF frames and the packed optical frames. Each counted daughter occupied at least 0.01% of the world, overlapped the preceding bank by at least 55% of its area and stayed separate for the next six states. The first optically confirmed event occurs at 0.083 s. The last two occur at 2.208 s near the right edge and 2.292 s near the left fifth of the gate. A further in-gate optical event at frame 21 has no independent source match. The different sampling resolutions shift the measured onset at some sites. [Matched sites](matched-sites.csv) · [Source detections](source-events.csv) · [Optical detections](optical-events.csv) · [Connectivity test](../verify.cpp)

The opening optical frame contains 35 detached bodies with centroids inside the gate, including oil already detached when the run begins. The median thresholded area is 6 world texels. At 1 s there are 46 bodies with a median of 9 texels. At the final frame there are 43 with a median of 11 texels. For the original Coral release front, the corresponding counts are 27, 26 and 27, with medians of 24, 24 and 26 texels.

The full-domain dyed fraction begins at 0.406372492921 and its largest relative deviation is 0.0001645%. The largest change in thresholded optical area is 128 of the opening 201,270 texels, or 0.0636%. [VOF samples](front-phase.csv) · [Optical area measurements](optical-events.csv)

## Reproduce the cache

Build the original coral experiment first, following its [prerequisites and instructions](../README.md#reproduction). The second command reuses that solver, packer, verifier and rear reference. It requires Ruby with zlib to extract the front byte from each packed texel.

```sh
experiments/coral-release/reproduce.sh /path/to/original-coral
experiments/coral-release/small-drops/reproduce.sh /path/to/original-coral /path/to/new-front
```

The generated `coral-small-front.r8.gz` decompresses to 96 consecutive 704 × 704 byte planes in row-major order. Byte zero is the front field's first texel. Codes of 128 and above fall on the oil side of the reconstructed contour. The file SHA-256 is `f45365cc3fb8808fb172d535cf15e31f237f9a661a57bf100df3b2b8524394e1`. [Bundled front field](../../../live-wheel/assets/coral-small-front.r8.gz)
