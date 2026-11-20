# Pressure on a sealed slide

The [four-second native sequence](slide-compression.mp4) holds Violet / Blue at absolute cache frame 30. The first two seconds press and release the rear slide. The second two press and release the front. Each neutral interval reproduces the original RGBA8 frame byte for byte. The software Vulkan device executed the production shader at 512 × 288. The video contains 96 frames at 24 fps. [Comparison](slide-comparison.png) · [Neutral](neutral.png) · [Rear pressed](rear-pressed.png) · [Front pressed](front-pressed.png)

## Sliding material and gap

In the study scale the visible glass gate measures 160 × 90 mm. Contact acts on one glass slide inside a circle of nominal radius 28.8 mm. The sealed world is larger than the projection gate. Contact near a sealed edge reduces the radius so its entire support remains inside the glass. The normalized control u is in [0,1]. Its maximum is a design setting. Converting a particular pen or touchpad reading into pascals would require the device force, contact area and glass compliance.

For the current radial position r, define s=(r/R)² and a=0.65u. The squared radius of the corresponding material at rest is s₀=s[1−a(1−s)³] inside the disk. Angle is preserved and the map is identity outside the disk. The area element is proportional to ds, so the inverse-map determinant is ds₀/ds=1−a(1−s)²(1−4s). Set the current physical gap to h=h₀ ds₀/ds. The forward map F then satisfies h(F(x)) det DF(x)=h₀ in **every material element**. Dye and carrier use the same map, so the equation applies to each phase. At full pressure the center reaches 0.35h₀. The accommodating annulus reaches 1.1625h₀ at R/√2. The boundary's displacement and first derivative match the unpressed map. The determinant stays strictly positive throughout.

A radial interaction in glass coordinates composes with the existing area-preserving tangential tug and rigid pose. The source sampler reverses rigid pose, compression and tug to recover original material. Track and slide selection isolate the two gaps. Releasing a press returns its factor to one while preserving prior tug and rotation. A current-pose export copies the contact state. A neutral export clears both contacts. Timed pressure samples can set either contact on the isolated movie track. [Pressure model](../../../../live-wheel/src/pressure.rs) · [Projected coordinate mapping](../../../../live-wheel/src/fields.wgsl)

## Optical effects and scope

The dyed phase's Beer–Lambert path on each aperture ray scales with its local gap. The clear carrier occupies the complementary conserved volume. The photographic wheel fit controls its base color. The thin-lens correction uses a nominal 1 mm neutral gap and a fixed source-side glass plate. Moving the liquid mid-plane changes its object distance by (h₀−h)/2. The resulting signed change to the blur radius adds to the wheel's existing pupil and focus setting. These are design dimensions. The model prescribes the reversible quasi-static response and does not estimate glass compliance, transient drag, immiscible interface breakup or a force-displacement curve from measurements. [Ray evaluation](../../optics.hook) · [Sampling and focus correction](../../../../live-wheel/src/footprint.glsl)

## Checks

The volume integral covers both phases across the full 704 × 704 source world at 1,982,464 subpixel midpoints per slide. Each sample uses the same fractional dye interface before and after the press. A finite image crop alone cannot measure sealed volume.

| Slide | Dyed projected area change | Dyed volume relative error | Carrier volume relative error |
| --- | ---: | ---: | ---: |
| Rear | +0.4774% | −0.0000233% | +0.0000083% |
| Front | +0.7483% | −0.0005445% | +0.0001477% |

A second integration runs the production GLSL source sampler and gap functions on the same 1408 × 1408 full-world grid. It reads back half-float dye and carrier volume from SwiftShader. The rear press changed the measured rear volumes by −0.0001061% and −0.0000185%. The front press changed its volumes by −0.0006195% and +0.0001240%. The opposite slide's two sums were identical in both experiments.

The native isolation check changed 20,468 rear field texels under rear load and 13,621 front field texels under front load. The opposite slide's sampled distance and its coordinate footprint were bit-identical. A known coordinate ramp under a quarter-turn matched the CPU pressure map within 0.519 of a single RG8 code. A uniform rear dye isolates optical path modulation from contour motion. At its center the green channel rose from 123 to 192, while the annulus fell from 108 to 96. Removing the pressure restored the exact neutral image. [Volume checks](../../../../live-wheel/src/pressure.rs) · [Native registration, optics and capture tests](../../../../live-wheel/src/pressure_test.rs)

From the repository root, run the Rust pressure unit tests with cargo test --locked --manifest-path live-wheel/Cargo.toml --lib pressure::tests. To execute the native tests, set VK_DRIVER_FILES to an isolated software Vulkan ICD manifest and TSS_PRESSURE_PROOF_DIR to a fresh empty directory. Run the following command. On macOS add --features wgpu/vulkan-portability before --lib. The test rejects any enumerated physical GPU.

```sh
cargo test --locked --manifest-path live-wheel/Cargo.toml --lib pressure_test -- --ignored --test-threads=1 --nocapture
```
