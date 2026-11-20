@group(0) @binding(0) var frames: texture_2d_array<f32>;
@group(0) @binding(1) var offsets: texture_2d<f32>;
@group(0) @binding(2) var smp: sampler;
struct FramePair { first:u32, second:u32, fraction:f32, pad:u32, rigid:vec4<f32>, centers:vec4<f32>, cacheRect:vec4<f32>, pressure:array<vec4<f32>,2> };
@group(0) @binding(3) var<uniform> pair: FramePair;
const aspect=vec2<f32>(1.77777778,1.0);
fn sourceUV(uv:vec2<f32>,rotation:vec2<f32>,origin:vec2<f32>,front:bool)->vec2<f32>{
    let press=pair.pressure[select(0u,1u,front)];
    var release=uv;
    if press.w > 0.0 {
        let delta=uv-press.xy;
        let q=delta*aspect;
        let s=dot(q,q)*press.z;
        if s < 1.0 {
            let t=1.0-s;
            release=press.xy+delta*sqrt(1.0-press.w*t*t*t);
        }
    }
    let p=(release-0.5)*aspect;
    let q=0.5+vec2<f32>(rotation.x*p.x-rotation.y*p.y, rotation.y*p.x+rotation.x*p.y)/aspect;
    let local=0.5+(q-origin)*aspect/0.64;
    let a=textureSampleLevel(offsets,smp,local,0.0);
    let delta=select(a.rg,a.ba,front);
    return (q+delta)*pair.cacheRect.xy+pair.cacheRect.zw;
}
fn phaseAt(uv:vec2<f32>,frame:u32,front:bool)->f32{
    let size=vec2<f32>(textureDimensions(frames));
    let edge=clamp(min(uv,1.0-uv)*size+0.5,vec2<f32>(0.0),vec2<f32>(1.0));
    let v=textureSampleLevel(frames,smp,uv,i32(frame),0.0);
    return select(v.r,v.g,front)*edge.x*edge.y;
}
fn footprint(q:vec2<f32>)->f32{
    let coord=q*vec2<f32>(textureDimensions(frames));
    let a=dpdx(coord);let b=dpdy(coord);
    let aa=dot(a,a);let bb=dot(b,b);let ab=dot(a,b);
    return sqrt(0.5*(aa+bb+sqrt((aa-bb)*(aa-bb)+4.0*ab*ab)));
}
@fragment fn main(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    let rear=sourceUV(uv,pair.rigid.xy,pair.centers.xy,false);
    let front=sourceUV(uv,pair.rigid.zw,pair.centers.zw,true);
    let a=vec2<f32>(phaseAt(rear,pair.first,false),phaseAt(front,pair.first,true));
    let b=vec2<f32>(phaseAt(rear,pair.second,false),phaseAt(front,pair.second,true));
    return vec4<f32>(mix(a,b,pair.fraction),footprint(rear),footprint(front));
}
