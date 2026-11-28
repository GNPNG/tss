@group(0) @binding(0) var field:texture_2d<f32>;
@group(0) @binding(1) var smp:sampler;
struct MaskOptics { lens: vec4<f32>, pressure: vec4<f32>, optics:vec4<f32>, frontPressure:vec4<f32> };
@group(0) @binding(2) var<uniform> maskOptics:MaskOptics;
fn phase(distance:f32)->f32 { return clamp((distance*255.-127.5)/3.+0.5,0.,1.); }
fn radius(local:vec2<f32>, nominal:f32, press:vec4<f32>)->vec2<f32>{
    let d = (local-press.xy)*vec2<f32>(16./9.,1.);
    let s = dot(d,d)*press.z;
    let t = max(1.-s,0.);
    let object = nominal + 0.5*press.w*t*t*(1.-4.*s);
    return maskOptics.optics.y*(1./object-1./maskOptics.lens.z)/vec2<f32>(1280.,720.);
}
fn rayCoverage(uv:vec2<f32>,ray:vec2<f32>,rear:vec2<f32>,front:vec2<f32>)->f32 {
    let a=phase(textureSampleLevel(field,smp,uv+ray*rear,0.).r);
    if maskOptics.optics.z==0. {return a;}
    return max(a,phase(textureSampleLevel(field,smp,uv+ray*front,0.).g));
}
@fragment fn oil_mask(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    let lens = maskOptics.lens;
    if (lens.y > 0.) {
        let local = vec2<f32>(0.5) + (uv-vec2<f32>(0.5))*lens.w;
        let rear = radius(local,lens.y,maskOptics.pressure);
        let front = radius(local,maskOptics.optics.x,maskOptics.frontPressure);
        // The optical pass uses 12 concentric strata with antipodal pupil rays.
        var coverage = 0.;
        for (var i=0; i<12; i=i+1) {
            let angle = 0.27+f32(i)*1.941611;
            let ray = vec2<f32>(cos(angle),sin(angle))*sqrt((f32(i)+0.5)/12.);
            coverage += rayCoverage(uv,ray,rear,front)+rayCoverage(uv,-ray,rear,front);
        }
        return vec4<f32>(coverage*lens.x/24.);
    }
    let sd=textureSampleLevel(field,smp,uv,0.).rg;
    return vec4<f32>(phase(max(sd.r,sd.g))*lens.x);
}
