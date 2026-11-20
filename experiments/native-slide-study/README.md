# Native slide-composition study

This isolated study uses the existing Vello 0.1.0 and wgpu 0.19.4 libraries to draw independently supplied Basilisk phase-fraction masks as transparent colored image layers into a native GPU texture. Vello's [v0.1.0 manifest](https://github.com/linebender/vello/blob/v0.1.0/Cargo.toml) specifies wgpu 0.19.3, compatible with 0.19.4. Vello supplies its own renderer and GPU programs; this study implements no fluid physics or custom GPU shaders.

The source is under development and has not been built or dispatched. It is a preliminary composition test: RGBA alpha blending is not validated projector transmission, and the input is a CPU-precomputed 2D two-fluid test rather than a calibrated confined-slide simulation. Its fixed colors and material parameters are only to identify independently supplied masks. It does not establish real-time physics, focus, compression, coalescence, 4K performance or user-approved appearance.

No GPU may be dispatched without coordinating the selected adapter, a single Windows job, existing Rust job guard, UUID/idle preflight and telemetry with the project lead. The browser is not involved.
