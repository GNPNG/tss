// Optional live path. The offline plate and reference optics use optics-header.glsl.
#define TSS_LIVE_FOOTPRINT 1
layout(set=0,binding=3) uniform texture2DArray sourceFrames;
layout(set=0,binding=4) uniform texture2D localWarp;
layout(set=0,binding=5,std140) uniform SourcePair {
    uint firstFrame;
    uint secondFrame;
    float timeFraction;
    float frontFraction;
    vec4 rotation;
    vec4 centers;
    vec4 cacheRect;
    vec4 pressure[2];
    uvec4 frontFrames;
    vec4 detail;
} source;
layout(set=0,binding=6) uniform texture2D filmMaterial;
const vec2 filmAspect = vec2(16.0/9.0, 1.0);
vec2 detailPoint(vec2 uv) {
    return source.detail.x == 1.0 ? uv : 0.5 + (uv - 0.5)*source.detail.x;
}
// The squared-radius inverse map s0=s[1-a(1-s)^3] conserves both phase
// volumes when the physical gap is h0 ds0/ds. Same inverse as fields.wgsl.
vec2 pressurePoint(vec2 uv,vec4 press) {
    if (press.w == 0.0) return uv;
    vec2 d=uv-press.xy;
    vec2 p=d*filmAspect;
    float s=dot(p,p)*press.z;
    if (s>=1.0) return uv;
    float t=1.0-s;
    return press.xy+d*sqrt(1.0-press.w*t*t*t);
}
float pressureGap(vec2 uv,bool front) {
    vec4 press=source.pressure[front ? 1 : 0];
    if (press.w == 0.0) return 1.0;
    vec2 p=(detailPoint(uv)-press.xy)*filmAspect;
    float s=dot(p,p)*press.z;
    float t=max(1.0-s,0.0);
    return 1.0-press.w*t*t*(1.0-4.0*s);
}
float pressureDefocus(vec2 uv,bool front) {
    // Nominal gap 1 mm, source-side glass fixed. The optical mid-plane moves by
    // half the gap change. Convert the new conjugate to design-pixel pupil radius.
    float ds=0.5*(1.0-pressureGap(uv,front));
    float s=mix(mat.geometry.x,mat.geometry.y,front ? mat.lens.w : mat.lens.z);
    return -mat.geometry.z*ds/(s*(s+ds));
}
vec4 rayMaterial(vec2 uv, bool front) {
    vec2 atlasSize = vec2(textureSize(sampler2D(filmMaterial,fieldSampler),0));
    vec2 inset = vec2(0.5,1.0)/atlasSize;
    vec2 p = clamp(uv,inset,1.0-inset);
    p.y = 0.5*(p.y+(front ? 1.0 : 0.0));
    return textureLod(sampler2D(filmMaterial,fieldSampler),p,0.0);
}

// Inverse pose and material-local displacement match fields.wgsl. Only the
// original field can supply phase coverage lost between intermediate texels.
vec2 filmSourceUV(vec2 uv, bool front) {
    vec2 cs = front ? source.rotation.zw : source.rotation.xy;
    vec2 centre = front ? source.centers.zw : source.centers.xy;
    vec2 p = (pressurePoint(detailPoint(uv),source.pressure[front ? 1 : 0]) - 0.5) * filmAspect;
    vec2 q = 0.5 + vec2(cs.x*p.x - cs.y*p.y, cs.y*p.x + cs.x*p.y) / filmAspect;
    vec2 localUV = 0.5 + (q-centre)*filmAspect/0.64;
    vec4 offset = textureLod(sampler2D(localWarp, fieldSampler),localUV,0.0);
    q += front ? offset.zw : offset.xy;
    return q * source.cacheRect.xy + source.cacheRect.zw;
}
float sourceDistance(vec2 uv, bool front) {
    vec2 p = filmSourceUV(uv,front);
    vec2 size = vec2(textureSize(sampler2DArray(sourceFrames,fieldSampler),0).xy);
    vec2 edge = clamp(min(p,1.0-p)*size+0.5,vec2(0.0),vec2(1.0));
    uint first = front ? source.frontFrames.x : source.firstFrame;
    uint second = front ? source.frontFrames.y : source.secondFrame;
    vec2 a = textureLod(sampler2DArray(sourceFrames,fieldSampler),vec3(p,float(first)),0.0).rg;
    vec2 b = textureLod(sampler2DArray(sourceFrames,fieldSampler),vec3(p,float(second)),0.0).rg;
    float d = front ? mix(a.g,b.g,source.frontFraction) : mix(a.r,b.r,source.timeFraction);
    return d*edge.x*edge.y;
}
vec2 changedFootprint(vec2 sigma, vec2 uv) {
    // An unrotated, uncompressed identity mapping has unit singular values.
    // A rigid turn can also move a thin interface between intermediate texels.
    vec2 turn = vec2(length(source.rotation.xy-vec2(1,0)),
                     length(source.rotation.zw-vec2(1,0)));
    turn=max(turn,vec2(abs(source.detail.x-1.0)));
    for (int k=0; k<2; ++k) {
        vec4 press=source.pressure[k];
        if(press.w>0.0) {
            vec2 p=(detailPoint(uv)-press.xy)*filmAspect;
            float t=max(1.0-dot(p,p)*press.z,0.0);
            turn[k]=max(turn[k],press.w*t*t*t);
        }
    }
    return max(smoothstep(vec2(1.025),vec2(1.20),sigma),
               smoothstep(vec2(0.0003),vec2(0.002),turn));
}
vec4 footprintField(vec2 p) {
    vec4 coarse = textureLod(sampler2D(field,fieldSampler),p,0.0);
    vec2 dist = abs((coarse.rg-0.5)*96.0);
    vec2 sigma = coarse.ba;
    vec2 guard = 12.0 + 3.0*sigma;
    vec2 change = changedFootprint(sigma,p);
    if (change.x > 0.0 && dist.x <= guard.x) coarse.r = sourceDistance(p,false);
    if (change.y > 0.0 && dist.y <= guard.y) coarse.g = sourceDistance(p,true);
    return coarse;
}
#undef HOOKED_tex
#define HOOKED_tex(point) footprintField(point)

// A finite circular pupil may reach an interface outside the central pixel.
// In the source distance units, bound that reach by projected magnification.
float footprintSupport(vec2 uv,float pupil,vec2 pixelStep) {
    vec4 f = textureLod(sampler2D(field,fieldSampler),uv,0.0);
    vec2 sigma = max(f.ba,vec2(1.0));
    vec2 dist = abs((f.rg-0.5)*96.0);
    float pixelWidth = max(pixelStep.x*1280.0,pixelStep.y*720.0);
    vec2 inner = 4.0+(pupil+pixelWidth+4.0)*sigma;
    vec2 nearRim = 1.0-smoothstep(inner,inner+3.0,dist);
    vec2 support = changedFootprint(f.ba,uv)*nearRim;
    return max(support.x,support.y);
}
vec2 pixelStratum(int i) {
    // One centered sample in every cell of a 6 x 4 pixel grid. The shuffled
    // antipodal pairs have zero centroid, like the independently sampled pupil.
    int k = (5*(i/2)) % 12;
    vec2 p = vec2((float(k%3)+0.5)/6.0,(float(k/3)+0.5)/4.0-0.5);
    return (i % 2 == 0) ? p : -p;
}
