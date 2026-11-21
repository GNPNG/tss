# MeltGL confined-slide suitability study

Status: investigation, not a selected TSS renderer or a claim of authentic projection. No GPU workload has been launched by this study.

The upstream project is [MeltGL](https://github.com/1etu/MeltGL/tree/c9a428421f6e4db6df70f8fc000ec9b764224ad6), commit `c9a428421f6e4db6df70f8fc000ec9b764224ad6`, by Ege Türker. Its [MIT license](https://github.com/1etu/MeltGL/blob/c9a428421f6e4db6df70f8fc000ec9b764224ad6/LICENSE) requires retention of the copyright and permission notice. Additional dependencies and component provenance still need review before redistribution.

The owner specifies distinct sandwiched glass slide planes, with no in-plane directional preference. Liquid pockets may merge within their own plane; separate planes cannot merge and overlap optically. Through-stack depth determines focus, and compression/pressure may affect plate spacing and liquid. User-supplied projected slide disc images are appearance references; the Cake23 example is evidence and a motion reference, not a mandated implementation.

Upstream describes its implementation as a **2D free-surface melting solver**, rather than confined oil and water between plates. It is being evaluated to establish which parts, if any, can fulfill the requested slide behavior. The checked [simulation source](https://github.com/1etu/MeltGL/blob/c9a428421f6e4db6df70f8fc000ec9b764224ad6/Packages/DisplacementEngine/src/simulation.ts) has separately updated per-layer GPU fields, a level-set boundary, curvature and tension controls, configurable gravity and material properties, simulation-resolution scaling and periodic snapshots. The renderer exposes translucency, absorption, tint, refraction and rim parameters.

The source initializes every layer from the same input alpha mask; distinct per-layer masks are not established. Within-layer physically calibrated coalescence, confined two-fluid dynamics, independently controlled pressure-dependent plate gaps, optical focus across planes and faithful transmission through the stack are unproven. Separate fields do not themselves establish calibration or authentic optics. The `step` method advances each layer using that layer's fields; this separation can prevent physical merging across independent planes, but visual and physical correctness remain to be demonstrated. User-facing shapes, controls and frame rate on the target hardware have not been tested.

## Windows build

The upstream checkout on the Windows test laptop is isolated at `%USERPROFILE%\work\tss-workspace\meltgl-study\source`. Its manifest declares Node >=20 and `pnpm@10.29.2`; its checked-in pnpm lockfile is present. The host initially exposed Node 22.23.4 but no npm or pnpm on PATH. The project-declared pnpm and compatible Node were installed and invoked via `mise x`; no global version selection was changed.

The official workspace install used `pnpm install --frozen-lockfile` with child concurrency 2 and network concurrency 4; the lockfile required no changes. pnpm reported that esbuild dependency lifecycle scripts were ignored under its normal policy. The subsequent build succeeded without changing that policy.

`pnpm run build --concurrency=2` completed 14 of 14 upstream workspace tasks. The single-worker upstream Vitest integration suite completed 6 files and 61 tests with no failures. These tests exercise options and application logic, not GPU rendering or the required physical film behavior. The build used Turborepo, which printed its anonymous telemetry notice; no project telemetry setting was changed. `git status --short` was clean after the build.

Commands, run with `mise x node@22.23.4 pnpm@10.29.2 --` on Windows:

```powershell
pnpm --dir "$env:USERPROFILE\work\tss-workspace\meltgl-study\source" install --frozen-lockfile --child-concurrency=2 --network-concurrency=4
pnpm --dir "$env:USERPROFILE\work\tss-workspace\meltgl-study\source" run build --concurrency=2
pnpm --dir "$env:USERPROFILE\work\tss-workspace\meltgl-study\source" --filter @meltgl/integration-tests exec vitest run --maxWorkers=1 --minWorkers=1
```

No browser, simulation workload or GPU dispatch was launched. These results do not establish a suitable renderer, appearance, focus, merging behavior or real-time performance.
