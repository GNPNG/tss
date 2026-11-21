# Release-target dependency inventory

The [manifest table](manifests.tsv) lists the dependency declarations of both Rust crates, including target predicates, features, default features, build dependencies and dev dependencies. The [package table](packages.tsv) records exact package versions and declared licences from the selected Rust 1.88 Cargo graphs. The two graphs use the same release manifest/lock snapshot at `6ca9c32083b8` with `tauri/custom-protocol`. Cargo selected 313 package/version pairs on Intel macOS and 349 on Windows x64 MSVC, with 378 in their union. The [research catalog index](catalog.tsv) is generated from [the master registry](../../../docs/library-catalog.json). Each row retains its own integration decision. Detailed assessment and limitations are in the [report](../../../docs/rust-native-assessment.md).

Run the following from the TSS checkout at the snapshot revision above with the locked registry packages cached for both release targets. The inventory reads both manifests and the catalog without building package code.

```sh
sh experiments/rust-native-assessment/inventory/build.sh
```

If the caches reside on separate hosts, run each target's two `cargo tree` commands shown in `build.sh` on that host from the same manifests and lock. Save the four outputs under `target/rust-assessment/inventory` as `mac-normal.tree`, `mac-all.tree`, `win-normal.tree`, `win-all.tree`, then run the script with `--existing-trees`. This procedure was used for the snapshot. On Windows use the installed MSVC toolchain's cargo executable if the rustup shim is unavailable.

The package key is Cargo name and version. An app-direct runtime dependency is marked `direct`. Other members of its normal graph are marked `transitive`. The normal view excludes procedural macros. A package reachable only through the broader graph is marked `build/proc-macro` or `direct build` for the root build dependency. If multiple paths reach a package, the normal dependency role takes precedence. A dash marks absence on that target. Dev/test-only packages are excluded from these two target graphs. Cargo's declared licence expression is a starting point for source and binary redistribution review. Native libraries, bundled assets, OS frameworks, externally installed codecs and separately launched tools require their own inventory. [Application boundaries](../../../docs/rust-native-assessment.md)
