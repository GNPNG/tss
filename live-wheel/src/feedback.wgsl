@group(0) @binding(0) var current: texture_2d<f32>;
@group(0) @binding(1) var history: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;
struct Loop { gain:f32, inverseZoom:f32, cosine:f32, sine:f32 };
@group(0) @binding(3) var<uniform> config: Loop;
@fragment fn feedback(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let base = textureSampleLevel(current,smp,uv,0.0).rgb;
    let aspect = vec2<f32>(16.0/9.0,1.0);
    let p = (uv-0.5)*aspect*config.inverseZoom;
    let q = 0.5+vec2<f32>(config.cosine*p.x-config.sine*p.y, config.sine*p.x+config.cosine*p.y)/aspect;
    let prior = textureSampleLevel(history,smp,q,0.0).rgb;
    let inside = all(q>=vec2<f32>(0.0)) && all(q<=vec2<f32>(1.0));
    return vec4<f32>(mix(base,prior,select(0.0,config.gain,inside)),1.0);
}
