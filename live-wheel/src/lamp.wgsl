@group(0) @binding(0) var<uniform> lampGain:vec4<f32>;
@fragment fn lamp(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    let d=(uv-vec2<f32>(0.56,0.43))*vec2<f32>(1.57,1.35);
    let rad=dot(d,d);
    let power=0.80/(1.+0.75*rad)+0.19*exp(-8.5*rad);
    let tint=mix(vec3<f32>(1.,0.949,0.907),vec3<f32>(0.786,0.761,0.918),smoothstep(0.036,0.58,rad));
    return vec4<f32>(tint*power*lampGain.rgb,1.);
}
@fragment fn dark(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{return vec4<f32>(0.,0.,0.,1.);}
