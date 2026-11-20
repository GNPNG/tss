# Paraxial projector lens reference

This CPU-only reference describes an ideal circular-aperture thin lens and a finite flat detector. It is a numerical reference for future TSS optical work; it is not connected to the renderer and does not reproduce or claim calibration against a particular projector. It creates no procedural liquid textures.

All positions, object-to-lens distance `s`, lens-to-screen distance `L`, focal length `f`, pupil radius and detector dimensions are in millimetres. Ray slopes are `dx/dz, dy/dz` (approximately angles in radians in the paraxial regime). For incident object-plane position `x` and slope `u`, the lens position is `p = x + s u`, the lens transmits the ray only if `|p| <= pupil radius`, and its outgoing slope is `u - p/f`. The screen position is `p + L(u - p/f)`. These equations apply independently in x and y.

For `s > f`, the real focused image is at `L = fs/(s-f)` with transverse magnification `-L/s`. For a fixed screen and a point at any positive `s`, the diameter of its geometric blur circle is `2 R |1 + L (1/s - 1/f)|`. A uniformly sampled circular pupil produces a uniform-area disk in that idealized model. Equal-area quadrature samples represent equal **relative admitted flux**; they do not determine absolute lamp power, radiance, etendue, illumination falloff or throughput changes as aperture size changes.

`detector` bins ray power per pixel and divides by pixel area in mm² for relative irradiance. It reports power outside the finite detector separately, with no normalization to disguise clipping. The lens trace returns a mask for blocked rays; callers must discard those rays before binning. It can accept measured or separately simulated ray positions and slopes, but **this study has not connected or calibrated the liquid simulation to millimetres or to the lens**.

The reference omits lens-element thickness and aberrations, diffraction, coherence, wavelength dependence, Fresnel losses, vignetting apart from its stop, obliquity/radiance weights, sensor or screen response, lamp geometry, reflectance, and a real projector's multiple optical elements. The planar exit position/slope interface alone does not validate the upstream 3D fluid geometry or ray tracing. Discrete pupil and detector sampling can introduce aliasing; zero blur means geometric focus in this approximation, not diffraction-limited focus.

The Gaussian thin-lens relation is summarized in [OpenStax University Physics, Geometric Optics](https://phys.libretexts.org/Bookshelves/University_Physics/University_Physics_%28OpenStax%29/University_Physics_III_-_Optics_and_Modern_Physics_%28OpenStax%29/02%3A_Geometric_Optics_and_Image_Formation/2.0S%3A_2.S%3A_Geometric_Optics_and_Image_Formation_%28Summary%29). [Physically Based Rendering, Projective Camera Models](https://pbr-book.org/4ed/Cameras_and_Film/Projective_Camera_Models) describes finite-aperture ray sampling and the circle of confusion; its camera discussion is used here only for the same thin-lens geometry, not as evidence of a calibrated projector.

Run the CPU tests with Python 3 and NumPy:

```sh
python -m unittest -v test_projector_lens
```

Tests cover focused off-axis image location and inversion, analytic out-of-focus diameter and pupil second moment, aperture rejection, finite-detector flux accounting per color channel, and invalid inputs. They do not validate a GPU, fluid solver, real-time performance, measured optical system or visual appearance.
