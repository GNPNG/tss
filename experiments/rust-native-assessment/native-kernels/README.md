# Native cue matching and optical shader compilation

Run `sh experiments/rust-native-assessment/native-kernels/verify.sh` with Cargo and rustc 1.88.0, a C++17 compiler, pkg-config, Vectorscan's libhs, glslc and spirv-val. Each run writes to a fresh directory under the ignored target tree.

On the Intel Mac, the C++ Vectorscan and Rust aho-corasick 1.1.5 executables produced the same four CSV rows as the committed phrase fixture. Each of the two literals matched at samples 48000–96000 and 192000–264000. The 75% gesture positions were 84000 and 246000. Production acoustic feature extraction, regex matching, throughput and peak memory were outside this comparison.

The production optical build composed ten GLSL fragment units. Naga 0.19.2 parsed all ten, validated their IR, and wrote SPIR-V 1.3 for each. Khronos spirv-val accepted all ten Naga outputs and all ten glslc outputs under Vulkan 1.1. Pixel parity, numerical error near an oil boundary, supported driver combinations and GPU time still require direct comparison. [Production shaders](../../../live-wheel/build.rs) · [Reference cue fixture](../../phrase-triggers/fixture.csv)
