# Thin-lens defocus experiment

This isolated calculation estimates geometric depth-of-field blur for a point imaged through an ideal thin converging lens. It is not integrated into TSS, does not render or blur images, and has not been calibrated against a projector or microscope. It does not establish an appropriate layer resolution or a speedup. Transparent materials, light transport and coupled fluid state need separate handling; filtering a flattened composite is not assumed equivalent to applying optics at the relevant depths.

`estimateDefocus` accepts positive finite `focalLength`, `objectDistance` and `screenDistance` in one consistent length unit. `objectDistance` must exceed `focalLength`, so the simplified model has a real image on the opposite side of the lens. `apertureDiameter` is a nonnegative finite diameter in the same units; zero is the ideal geometric pinhole limit. `pixelsPerUnit` is a positive finite output-pixel scale for that unit at the screen. The returned `imageDistance` and `circleDiameter` use the supplied length unit; `circleDiameterPixels` is a dimensionless count of output pixels.

The model uses `1/f = 1/s + 1/v` and `c = D*abs(1 - screenDistance/v)`, where `v` is the image distance of the selected object plane, `D` is aperture diameter, and `c` is geometric circle-of-confusion diameter on the screen. It rejects non-finite inputs, virtual-image cases and non-finite calculated results. It does not model aperture shape, lens aberrations, diffraction, refractive liquid interfaces, non-planar objects or microscope numerical aperture. Real lens and layer measurements are not known for the user's references. See [Barsky et al., Camera Models and Optical Systems Used in Computer Graphics](https://graphics.berkeley.edu/papers/Barsky-CMO-2003-05/Barsky-CMO-2003-05.pdf).

Run from the TSS repository root:

```sh
mise x node@24.15.0 -- node --test experiments/optical-focus/thin-lens.test.mjs
```

Tests use independent lens-equation examples, similar triangles on either side of focus, aperture and pixel scaling, unit conversion, invalid and non-finite values, and an overflow-resistant algebraic form. Evaluating a visibility or resolution heuristic still requires image-quality and GPU performance tests on actual layered scenes.
