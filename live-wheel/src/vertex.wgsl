struct Vertex {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex fn main(@builtin(vertex_index) i: u32) -> Vertex {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Vertex(vec4<f32>(2.0*p.x-1.0, 1.0-2.0*p.y, 0.0, 1.0), p);
}
