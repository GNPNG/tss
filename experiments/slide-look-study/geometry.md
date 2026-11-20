# Initial slide geometry

The violet and cyan files each describe the initial lateral footprint of one dyed phase. They are independent art-directed initial conditions for the solver. White is dyed liquid and black is clear carrier. Antialiased samples at an interface represent fractional coverage. The image axes run rightward and downward.

Each group of control points in `geometry.hpp` defines a closed uniform cubic B-spline. Its artist-set control cage determines the broad connected fingers, channels and detached lobes. Separate groups position isolated droplets and clear inclusions. A fixed seed determines each small inclusion cluster. All subsequent interface motion belongs to the selected solver.

From this directory on a machine with a C++17 compiler, librsvg and ImageMagick 7:

```sh
c++ -std=c++17 -O2 geometry-export.cpp -o geometry-export
for plate in violet cyan; do
    ./geometry-export "$plate" > "$plate.svg"
    rsvg-convert -w 1280 -h 720 -o "$plate.png" "$plate.svg"
    magick "$plate.png" -colorspace Gray -depth 8 "pgm:$plate.pgm"
done
```

The two independently rasterized 1280 × 720 P5 files used in this study have SHA-256 digests:

| Initial phase mask | SHA-256 |
| --- | --- |
| violet.pgm | `1df7728f159760c673edcc27036743d92648313dd5161417b80099a5831197b2` |
| cyan.pgm | `4263dba03a85f0270bf8e1d5ece4d7c81dc7685a5bd25d508b6e579c94d797d5` |

The initial fill fractions are 0.540 violet and 0.405 cyan. The mean product of the two masks is 0.160. This is the projected overlap across the two independent depths.
