struct PlateMaterial {
    rear: vec4<f32>, front: vec4<f32>, joint: vec4<f32>,
    rim: vec4<f32>, carrier: vec4<f32>, lens: vec4<f32>,
    // Scale and origin in the native 16:9 optical gate.
    gate: vec4<f32>,
}
@group(0) @binding(2) var<uniform> plate: PlateMaterial;

struct Vertex {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn main(@builtin(vertex_index) i: u32) -> Vertex {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Vertex(vec4<f32>(2.0*p.x-1.0, 1.0-2.0*p.y, 0.0, 1.0), p*plate.gate.xy+plate.gate.zw);
}
