# Local visual advice

The DJ selects a timeline region, enters a visual direction and optionally includes reviewed lyric cues that overlap that region. **Suggest visual layer** consults the local FunctionGemma service. Review the layer, signal and fit independently. The editor retains the DJ's original wording and time range. **Add reviewed oil layer** submits reviewed settings to the timeline. A changed song or selection requires another review.

The service uses the GNPNG visual-advice scenario in [local-advisor](https://gitea.gnomatix.com/gnox/local-advisor/src/branch/main/use-cases/tss-visual-advice). Run its pinned llama.cpp server and FunctionGemma model under the alias `local-advisor:functiongemma`. It defaults to loopback port 18081. The [runtime instructions](https://gitea.gnomatix.com/gnox/local-advisor/src/branch/main/research/functiongemma) give the pinned weights, upstream runtime and CPU service configuration. No audio leaves TSS. A request includes the direction and any lyric excerpt explicitly selected by the DJ.

## Measured behavior

The original [six native-call fixtures](https://gitea.gnomatix.com/gnox/local-advisor/src/commit/77f5c6a/use-cases/tss-visual-advice/evidence/native-base-intel/README.md) produced zero exact matches. The desktop adapter consults three small function catalogs, consumes exactly one allowed call per catalog and sends no editable numeric boundaries to the model. Each request stops at the native call terminator.

Against five further directions, the compiled editor adapter returned the expected category in 10 of 15 decisions. It got all three choices right for two directions. It misclassified paper triangles as a silhouette, and selected fixed duration for two requests to return on each occurrence. The operator can correct every decision before adding an oil track. The test measures visual-intent routing on five short directions. Image generation and suitability for a particular song require separate evidence. [Expected and actual choices](intel-functiongemma.json)

To repeat, use model SHA-256 `83940d4dd9676710856f43523bed096164a595a96f6b34771610a03937de5270`, llama.cpp b12110, two CPU threads, 2048-token context and one inference slot, then run the Rust evaluation against the local endpoint. Choose a new report filename for each run.

```sh
TSS_ADVICE_REPORT=/path/to/new-report.json cargo test --manifest-path src-tauri/Cargo.toml advisor::tests::evaluate_local_runtime -- --exact --ignored
```
