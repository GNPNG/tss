# Periodic subcell interface study

This CPU-only experiment reconstructs an approximate interface from the *actual* two-component Sailfish density samples already retained in the optical input. It does not change the fluid simulation, add droplets, reconstruct a solved 3-D interface, calibrate the optical model, or establish a realistic appearance. It is not integrated into the app.

The existing optical preparation thresholds `rho > phi`, computes the signed Euclidean distance to the binary phase-mask pixels, and applies one cell of Gaussian smoothing. This study instead uses [scikit-image's marching squares](https://scikit-image.org/docs/0.25.x/api/skimage.measure.html#skimage.measure.find_contours) on the zero level of `rho - phi` over a 3×3 periodic tile. It uses [SciPy's cKDTree](https://docs.scipy.org/doc/scipy/reference/generated/scipy.spatial.cKDTree.html) to locate candidate line segments, then calculates the nearest point on those segments. The midpoint search bound includes the maximum segment half-length, so it does not assume the nearest segment midpoint belongs to the nearest segment. Distances use the same domain-height units and sign convention as the existing experiment.

Interpolated contour geometry is a model of the sampled data. Marching squares follows the library's default ambiguity resolution for saddle cells. Interface topology, phase volume, temporal continuity, physical mixing, 3-D shape and the effect of omitting smoothing are not validated. Fine features below the lattice scale cannot be recovered from these fields. The implementation is a CPU prototype, not a performance result or production method.

## Reproduce

Python 3.11, NumPy 2.2.3, SciPy 1.15.2 and scikit-image 0.25.2 were used. Install them into an isolated Python environment. From this directory:

```sh
python -m unittest -v test_reconstruct
python evaluate.py ../optics-next/output/windows-two-vessel-input.f32le --frame 0 --vessel 0 --output /path/to/new-comparison.npz
python prepare_pair.py ../optics-next/output/windows-two-vessel-input.f32le /path/to/new-output-directory --frame 0
```

Outputs must not already exist. The input is the preserved two-vessel 59-frame optical dataset, 160×160, with SHA-256 `e1a5ac67c5135e63f70c81ef2e317376b69044d64cda882ecaca60ac6fe88249`. The evaluation makes copies of its input phase densities and writes comparisons without modifying the input. The input must have the documented exact size; it is not a general snapshot reader.

`prepare_pair.py` creates two single-frame `[1,10,160,160]` little-endian float32 renderer inputs. It changes only the first two signed-distance channels. Both density fields and the independent dye channels are bit-identical between the paired inputs, including signed-zero bit patterns. A manifest gives hashes and channel order. The tests cover periodic planes and circles, translated wraparound, grid refinement, invalid inputs, and bitwise preservation of the other eight channels. They do not test visual fidelity or real-world calibration.

## Recorded comparison

For frame 0, the mean absolute signed-distance differences between this approach and the existing smoothed approach were 0.0020254 and 0.0020243 domain heights for vessels 0 and 1; their maxima were 0.0069023 and 0.0051019. The original EDT method reproduced the saved precomputed distance values to within 7.4e-9 domain heights for the sampled frames. The reconstructed distance had no phase-sign disagreements at sampled grid points in the checked frame/vessel pairs (frame 0 in both vessels and frame 29 in vessel 0). These numbers are differences between two conventions, not measured errors against real fluids.

The locally generated pair is retained under ignored `output/frame-0000/`; its manifest specifies all hashes. The original is `46bf6cb54cf78dda715405aa6563e033237c8463e7b46c60cd9d57993addb1ab` and the subcell variant is `1f86aa8ff1a57617b62bf1457685a726f93569a1976bead116d80365434f2ce4`. The two reconstruction methods differ in *both* how the boundary is located and the absence/presence of smoothing; any image differences cannot be attributed solely to subcell interpolation.
