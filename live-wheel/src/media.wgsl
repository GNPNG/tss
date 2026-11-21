@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var prior: texture_2d<f32>;
@group(0) @binding(2) var maskA: texture_2d<f32>;
@group(0) @binding(3) var maskB: texture_2d<f32>;
@group(0) @binding(4) var smp: sampler;
struct Params { clock:vec4<f32>, key:vec4<f32>, colorA:vec4<f32>, colorB:vec4<f32>,
    motion:vec4<f32>, masks:vec4<f32>, audio:vec4<f32>, flags:vec4<f32> };
@group(0) @binding(5) var<uniform> cfg: Params;
fn linear(c:vec3<f32>)->vec3<f32>{
    return select(pow((c+0.055)/1.055,vec3<f32>(2.4)),c/12.92,c<=vec3<f32>(0.04045));
}
fn srgb(c:vec3<f32>)->vec3<f32>{
    return select(1.055*pow(max(c,vec3<f32>(0.)),vec3<f32>(1./2.4))-0.055,12.92*c,c<=vec3<f32>(0.0031308));
}
fn sample_media(uv:vec2<f32>)->vec4<f32>{
    if any(uv<vec2<f32>(0.)) || any(uv>vec2<f32>(1.)) {return vec4<f32>(0.);}
    // GPU filtering uses premultiplied linear RGB, preserving transparent edges.
    let size=vec2<i32>(textureDimensions(source));
    let pos=uv*vec2<f32>(size)-0.5;
    let idx=vec2<i32>(floor(pos));
    let f=fract(pos);
    var px=vec4<f32>(0.);
    for(var y=0;y<2;y++){ for(var x=0;x<2;x++){
        let sample=textureLoad(source,clamp(idx+vec2<i32>(x,y),vec2<i32>(0),size-vec2<i32>(1)),0);
        let weight=select(1.-f.x,f.x,x==1)*select(1.-f.y,f.y,y==1);
        px+=vec4<f32>(sample.rgb*sample.a,sample.a)*weight;
    }}
    if px.a<=0.00001 {return vec4<f32>(0.);}
    var color=px.rgb/px.a;
    let encoded=srgb(color);
    var alpha=px.a;
    if cfg.flags.x==1. || cfg.flags.z==1. {
        let delta=distance(encoded,cfg.key.rgb)*0.577350269;
        let key=smoothstep(cfg.key.w,cfg.key.w+max(cfg.motion.w,0.00001),delta);
        alpha*=key;
    }
    if cfg.flags.x==2. {color=linear(cfg.colorA.rgb);}
    if cfg.flags.x==3. {
        let lum=dot(color,vec3<f32>(0.2126,0.7152,0.0722));
        let wave=0.5-0.5*cos(6.2831853*(lum+uv.x*0.25+cfg.clock.x*cfg.colorA.w));
        color=mix(linear(cfg.colorA.rgb),linear(cfg.colorB.rgb),wave);
    }
    if cfg.flags.x==4. {
        let hue=fract(uv.y*0.8+dot(color,vec3<f32>(0.2126,0.7152,0.0722))*0.2+cfg.clock.x*cfg.colorA.w);
        let rgb=clamp(abs(fract(hue+vec3<f32>(0.,2./3.,1./3.))*6.-3.)-1.,vec3<f32>(0.),vec3<f32>(1.));
        color=linear(rgb);
    }
    let q=2.*(uv-0.5);
    var boundary=-1.;
    if cfg.flags.y==1. {boundary=length(q)-1.;}
    if cfg.flags.y==2. {boundary=abs(q.x)+abs(q.y)-1.;}
    if cfg.flags.y==3. || cfg.flags.y==4. {
        let arc=3.14159265/5.;
        let angle=atan2(q.y,q.x)+1.5707963;
        let theta=abs(fract((angle+arc)/(2.*arc))*2.*arc-arc);
        let inner=.42;
        let radius=inner*sin(arc)/(inner*sin(arc-theta)+sin(theta));
        if cfg.flags.y==3. {
            boundary=length(q)-radius;
        } else {
            // Equal log-radius intervals represent successive stars in perspective.
            let radial=length(q)/radius;
            let phase=cfg.flags.w*(cfg.clock.x+cfg.colorB.w*cfg.audio.w);
            let cycles=phase-log2(max(radial,0.0001));
            let band=0.5+0.5*cos(6.2831853*cycles);
            let pixel=cfg.clock.w/max(length(q),0.0001);
            let ring=smoothstep(0.35-min(pixel,0.3),0.35+min(pixel,0.3),band);
            alpha*=mix(ring,0.5,smoothstep(0.2,1.,pixel));
        }
    }
    alpha*=1.-smoothstep(-cfg.clock.w,cfg.clock.w,boundary);
    return vec4<f32>(color*alpha,alpha);
}
fn artwork(uv:vec2<f32>)->vec4<f32>{
    let aspect=cfg.clock.z;
    var p=uv;
    p.y+=cfg.motion.x*0.12*(0.5+0.5*sin(6.28318*cfg.clock.x))*(0.2+0.8*cfg.audio.x);
    p.x+=cfg.motion.z*0.065*sin(12.*p.y+5.*cfg.clock.x)*(0.1+0.9*cfg.audio.y);
    p=(p-0.5)*vec2<f32>(max(aspect,1.),max(1./aspect,1.))+0.5;
    let blur=cfg.motion.y*(0.1+0.9*cfg.audio.z)*0.018;
    var col=sample_media(p);
    if blur>0.00001 {
        col=(col*4.+sample_media(p+vec2<f32>(blur,0.))+sample_media(p-vec2<f32>(blur,0.))+
            sample_media(p+vec2<f32>(0.,blur))+sample_media(p-vec2<f32>(0.,blur)))/8.;
    }
    return col*cfg.clock.y;
}
@fragment fn mask(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    return vec4<f32>(artwork(uv).a);
}
@fragment fn compose(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
    var a=textureSampleLevel(maskA,smp,uv,0.).r;
    let b=textureSampleLevel(maskB,smp,uv,0.).r;
    if cfg.masks.x==0. {a=select(1.,b,cfg.masks.y==1.);}
    else if cfg.masks.y==1. {
        if cfg.masks.z==0. {a=min(a,b);}
        if cfg.masks.z==1. {a=max(a,b);}
        if cfg.masks.z==2. {a=max(0.,a-b);}
        if cfg.masks.z==3. {a=abs(a-b);}
    }
    let c=artwork(uv)*a;
    let under=textureSampleLevel(prior,smp,uv,0.).rgb;
    return vec4<f32>(c.rgb+(1.-c.a)*under,1.);
}
