@fragment fn lamp(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    let d=(uv-vec2<f32>(0.56,0.43))*vec2<f32>(1.57,1.35);
    let rad=dot(d,d);
    let power=0.80/(1.+0.75*rad)+0.19*exp(-8.5*rad);
    let tint=mix(vec3<f32>(1.,0.949,0.907),vec3<f32>(0.786,0.761,0.918),smoothstep(0.036,0.58,rad));
    return vec4<f32>(tint*power,1.);
}
@fragment fn dark(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{return vec4<f32>(0.,0.,0.,1.);}
@group(0) @binding(0) var field:texture_2d<f32>;
@group(0) @binding(1) var smp:sampler;
@group(0) @binding(2) var<uniform> opacity:vec4<f32>;
@fragment fn oil_mask(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    let sd=textureSampleLevel(field,smp,uv,0.).rg;
    let coverage=clamp((max(sd.r,sd.g)*255.-127.5)/3.+0.5,0.,1.);
    return vec4<f32>(coverage*opacity.x);
}
