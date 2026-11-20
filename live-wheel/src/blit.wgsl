@group(0) @binding(0) var optics: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@fragment fn copy_encoded(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSampleLevel(optics,smp,uv,0.0);
}
@fragment fn decode_for_srgb(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let encoded=textureSampleLevel(optics,smp,uv,0.0).rgb;
    let linear=select(pow((encoded+0.055)/1.055,vec3<f32>(2.4)),encoded/12.92,encoded<vec3<f32>(0.04045));
    return vec4<f32>(linear,1.0);
}
