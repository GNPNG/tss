@group(0) @binding(0) var program: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@fragment fn keep_linear(@location(0) uv:vec2<f32>)->@location(0) vec4<f32> {
    return textureSampleLevel(program,smp,uv,0.0);
}
@fragment fn encode_program(@location(0) uv:vec2<f32>)->@location(0) vec4<f32> {
    let rgb=clamp(textureSampleLevel(program,smp,uv,0.0).rgb,vec3<f32>(0.0),vec3<f32>(1.0));
    return vec4<f32>(select(1.055*pow(rgb,vec3<f32>(1.0/2.4))-0.055,12.92*rgb,rgb<=vec3<f32>(0.0031308)),1.0);
}
