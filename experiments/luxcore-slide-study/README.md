# Static LuxCore optics experiment — failed visual target

This isolated scene tests LuxCore's glass, clear absorbing media, and overlapping geometry. **It is not a fluid simulation, a TSS renderer, an animation, or an approved visual.** The scene uses four manually positioned, shallow ellipsoids at two separated depths between static glass slabs. Their geometry does not match the Mathmos references; increasing samples alone will not fix that failure. The study was stopped after visual review, and no GPU job was run for this scene.

`Mirror.ply` is the sphere mesh from [LuxCore source commit b872186](https://github.com/LuxCoreRender/LuxCore/blob/b872186/scenes/sky/Mirror.ply), under the upstream project license. `plate.ply` is a static rectangular slab. `slide.scn` configures LuxCore's existing `glass` materials and `clear` absorbing volumes; no renderer or fluid physics algorithm was implemented in this experiment.

The test ran on the Windows test laptop using the existing LuxCore 2.11.2 sample console and existing Rust GPU job guard. `cpu-check.cmd` invokes `PATHCPU` with `native.threads.count=1`, `film.hw.enable=0`, `batch.haltspp=2`, 128×128 output, and NVIDIA sampled-idle preflight/telemetry. The actual LuxCore log reports one native CPU intersection device; the two telemetry samples each report NVIDIA utilization 0% and memory 0 MiB. The guard released cleanly. These samples do not prove the absence of all GPU activity.

The first parse failed with `Material re-definition: plate` and `Bad dynamic_cast!` because the scene gave a material and a volume the same name. Material identifiers were corrected; `cpu-check.log` preserves the original failure. The second execution produced `cpu-check-02.png` and `cpu-check-02.log`. The image is heavily noisy and depicts static primitive ellipsoids, not the reference's actual moving interfaces.

SHA-256:

- `Mirror.ply`: `cb613418adeb360b24641e25d8070e5da57e6dcbf24ff9463a3aa5250cd4c9cf`
- `plate.ply`: `68d8444c9a230fecaa1fd623ae96961ffe59427a777ac30a315192860c856d40`
- `slide.scn`: `92d0eea7a8eb4afb54b62c92229abff27b04069ba069f88348ca4ce1476791ea`
- `slide.cfg`: `15d4bf6daab73b1122e209277fdf158737cf066745c467ba72c0d768f271c389`
- `cpu-check-02.png`: `16019b32aa47637998e0cb6bd3be35ac20960aa98edfd6dc5c40945ec60ac6b3`

Future work requires an independently simulated confined liquid interface for each slide layer, within-layer coalescence, through-stack optics/focus and user-approved motion. None was demonstrated here.
