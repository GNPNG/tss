// A compact pair of radial twists models a material-coordinate pull. Each
// twist adds a function of radius to polar angle and preserves area.
struct Tug { rear: vec4<f32>, front: vec4<f32> };
@group(0) @binding(0) var<uniform> tug: Tug;
const aspect=vec2<f32>(1.77777778,1.0);
fn twist(p:vec2<f32>,center:vec2<f32>,angle:f32)->vec2<f32>{
    let v=p-center;
    let w=max(0.0,1.0-dot(v,v)/(0.23*0.23));
    let turn=angle*w*w*w;
    let cs=cos(turn);let sn=sin(turn);
    return center+vec2<f32>(cs*v.x-sn*v.y,sn*v.x+cs*v.y);
}
fn offset(p:vec2<f32>,strength:vec2<f32>)->vec2<f32>{
    let amplitude=length(strength);
    if amplitude < 0.0000001 { return vec2<f32>(0.0); }
    let axis=vec2<f32>(-strength.y,strength.x)/amplitude*0.050;
    if min(distance(p,axis),distance(p,-axis))>=0.23{return vec2<f32>(0.0);}
    let q=twist(twist(p,axis,-amplitude),-axis,amplitude);
    return (q-p)/aspect;
}
@fragment fn main(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    let p=(uv-0.5)*0.64;
    return vec4<f32>(offset(p,tug.rear.xy),offset(p,tug.front.xy));
}
