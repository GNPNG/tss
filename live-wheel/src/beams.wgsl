@group(0) @binding(0) var first: texture_2d<f32>;
@group(0) @binding(1) var second: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;
@group(0) @binding(3) var<uniform> settings: vec4<f32>;

@fragment fn add_beams(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let primary = textureSampleLevel(first, smp, uv, 0.).rgb * settings.z;
    let at = uv - settings.xy;
    let inside = all(at >= vec2<f32>(0.)) && all(at <= vec2<f32>(1.));
    let other = textureSampleLevel(second, smp, at, 0.).rgb;
    return vec4<f32>(primary + select(vec3<f32>(0.), other * settings.w, inside), 1.);
}
