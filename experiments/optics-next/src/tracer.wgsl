// Thin-cell optical study, not a calibrated reproduction of a real projector.
// A 2-D computed interface is lofted through the gap as a circular arc.
// The medium on one side is dyed and absorbs light by Beer–Lambert; the
// other side is clear. A finite pupil rejects strongly deviated rays.
struct Params {
    output_width: u32, output_height: u32, field_width: u32, field_height: u32,
    layers: u32, phase_inversions: u32, pad1: u32, pad2: u32,
    absorption0: vec4<f32>, absorption1: vec4<f32>,
    absorption2: vec4<f32>, absorption3: vec4<f32>,
    optical: vec4<f32>, // gap/domain-height, n clear, n dyed, pupil slope
}
@group(0) @binding(0) var<storage, read> field: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> p: Params;

fn at(i: u32, x: i32, y: i32) -> f32 {
    let w=i32(p.field_width); let h=i32(p.field_height);
    let xx = u32(((x % w) + w) % w);
    let yy = u32(((y % h) + h) % h);
    return field[i * p.field_width * p.field_height + yy * p.field_width + xx];
}

// q.x and q.y are in physical domain-height units; x spans its aspect ratio.
fn sdf(i: u32, q: vec2<f32>) -> f32 {
    let aspect = f32(p.field_width) / f32(p.field_height);
    let uv = fract(vec2<f32>(q.x / aspect, q.y));
    let xy = uv * vec2<f32>(f32(p.field_width), f32(p.field_height));
    let b = vec2<i32>(floor(xy));
    let a = fract(xy);
    return mix(mix(at(i,b.x,b.y),at(i,b.x+1,b.y),a.x),
               mix(at(i,b.x,b.y+1),at(i,b.x+1,b.y+1),a.x),a.y);
}
fn fluid_density(i:u32,phase:u32,q:vec2<f32>) -> f32 {
    return max(sdf(i+(phase+1u)*p.layers,q),0.0);
}
fn absorb(i:u32) -> vec3<f32> {
    switch i {
      case 0u:{return p.absorption0.xyz;} case 1u:{return p.absorption1.xyz;}
      case 2u:{return p.absorption2.xyz;} default:{return p.absorption3.xyz;}
    }
}
fn focus_radius(i:u32)->f32 {
    if(i==0u){return p.absorption0.w;}
    return p.absorption2.w;
}
// A chosen contact angle of 35 degrees fixes the circular-arc radius.  It is
// a modeled equilibrium profile, not a 3-D solver result or a measurement.
fn arc(z: f32) -> f32 {
    let h=p.optical.x; let radius=h/(2.0*0.8191520443);
    let v=z-h*0.5;
    return sqrt(max(radius*radius-v*v,1e-12)) - sqrt(max(radius*radius-h*h*0.25,1e-12));
}
fn arc_derivative(z:f32) -> f32 {
    let h=p.optical.x; let radius=h/(2.0*0.8191520443); let v=z-h*0.5;
    return -v/sqrt(max(radius*radius-v*v,1e-12));
}
fn fval(i:u32,q:vec2<f32>,z:f32) -> f32 { return sdf(i,q)+arc(z); }

struct RayResult { light:vec3<f32>, stats:vec3<f32> }
fn trace_ray(start:vec2<f32>, initial_slope:vec2<f32>) -> RayResult {
    var q=start;
    var dir=normalize(vec3<f32>(initial_slope,1.0));
    var light=vec3<f32>(1.0);
    var hits=0.0; var tir=0.0;
    let h=p.optical.x; let dz=h/24.0; let dx=1.0/f32(p.field_height);
    for(var layer=0u;layer<p.layers;layer=layer+1u){
       // Approximate defocus by integrating the independently specified
       // projected focal footprint of each vessel over source angles.  This
       // is not a calibrated multi-plane projector optical system.
       let fieldaspect=f32(p.field_width)/f32(p.field_height);
       let outaspect=f32(p.output_width)/f32(p.output_height);
       let px=fieldaspect/max(outaspect,fieldaspect)/f32(p.output_height);
       let focus_shift=(initial_slope/0.065)*focus_radius(layer)*px;
       q+=focus_shift;
       let invert=(p.phase_inversions & (1u<<layer))!=0u;
       var z=0.0;
       var inside=(fval(layer,q,z)>0.0)!=invert;
       var optical_depth=vec2<f32>(0.0);
       for(var step=0;step<24;step=step+1){
          if(!(dir.z>1e-5)){return RayResult(vec3<f32>(0.0),vec3<f32>(hits/8.0,tir/4.0,0.0));}
          let q0=q; let z0=z; let incoming_z=dir.z;
          var before_length=dz/incoming_z; var after_length=0.0;
          let nextq=q+dir.xy/dir.z*dz;
          let z1=z+dz;
          let nextinside=(fval(layer,nextq,z1)>0.0)!=invert;
          
          if(nextinside!=inside){
             hits+=1.0;
             // Interface crossings are bracketed, not assumed to be a true
             // signed-distance bound suitable for sphere tracing.
             var lo=0.0; var hi=1.0;
             for(var j=0;j<7;j=j+1){
                let mid=(lo+hi)*0.5;
                let sign=(fval(layer,mix(q0,nextq,mid),mix(z0,z1,mid))>0.0)!=invert;
                if(sign==inside){lo=mid;}else{hi=mid;}
             }
             let t=(lo+hi)*0.5; let hit=mix(q0,nextq,t); let hz=mix(z0,z1,t);
             before_length=t*dz/incoming_z;
             let gx=(sdf(layer,hit+vec2<f32>(dx,0.0))-sdf(layer,hit-vec2<f32>(dx,0.0)))/(2.0*dx);
             let gy=(sdf(layer,hit+vec2<f32>(0.0,dx))-sdf(layer,hit-vec2<f32>(0.0,dx)))/(2.0*dx);
             let gradient=vec3<f32>(gx,gy,arc_derivative(hz));
             let normal_squared=dot(gradient,gradient);
             if(!(normal_squared>1e-12)){return RayResult(vec3<f32>(0.0),vec3<f32>(hits/8.0,tir/4.0,0.0));}
             var normal=gradient*inverseSqrt(normal_squared);
             if(dot(normal,dir)>0.0){normal=-normal;}
             let n1=select(p.optical.y,p.optical.z,inside);
             let n2=select(p.optical.y,p.optical.z,nextinside);
             if(abs(n1-n2)>1e-7){
             let eta=n1/n2;
             let cosi=clamp(-dot(dir,normal),0.0,1.0);
             let sin2t=eta*eta*max(0.0,1.0-cosi*cosi);
             if(sin2t>=1.0){return RayResult(vec3<f32>(0.0),vec3<f32>(hits/8.0,(tir+1.0)/4.0,0.0));}
             let cost=sqrt(max(0.0,1.0-sin2t));
             let rs=(n1*cosi-n2*cost)/(n1*cosi+n2*cost);
             let rp=(n2*cosi-n1*cost)/(n2*cosi+n1*cost);
             let fresnel=0.5*(rs*rs+rp*rp);
             dir=normalize(eta*dir+(eta*cosi-cost)*normal);
             if(!(dir.z>1e-5)){return RayResult(vec3<f32>(0.0),vec3<f32>(hits/8.0,tir/4.0,0.0));}
             light*=1.0-fresnel;
             }
             after_length=(1.0-t)*dz/dir.z;
             q=hit+dir.xy/dir.z*dz*(1.0-t);
          }else{q=nextq;}
          // Solved density is used as an assumed concentration proxy: no
          // independently transported dye field exists in these inputs.
          let midq=(q0+q)*0.5;
          let rho=fluid_density(layer,0u,midq); let phi=fluid_density(layer,1u,midq);
          let total=rho+phi;
          if(!(total>1e-8)){return RayResult(vec3<f32>(0.0),vec3<f32>(hits/8.0,tir/4.0,0.0));}
          let alpha=clamp(rho/total,1e-4,1.0-1e-4);
          let before_is_a=inside!=invert;
          let a_length=select(after_length,before_length,before_is_a);
          let b_length=select(before_length,after_length,before_is_a);
          let dye_a=fluid_density(layer,2u,midq); let dye_b=fluid_density(layer,3u,midq);
          optical_depth+=vec2<f32>((dye_a/alpha)*a_length,(dye_b/(1.0-alpha))*b_length)/h;
          z=z1; inside=(fval(layer,q,z)>0.0)!=invert;
       }
       // Both library-evolved component densities absorb independently in the
       // same vessel. Dye concentration is assumed proportional to species
       // density; the solver does not independently transport actual dye.
       light*=exp(-absorb(layer*2u)*optical_depth.x-absorb(layer*2u+1u)*optical_depth.y);
       q-=focus_shift;
    }
    if(!(dir.z>1e-5)){return RayResult(vec3<f32>(0.0),vec3<f32>(hits/8.0,tir/4.0,0.0));}
    let slope=length(dir.xy)/dir.z;
    let pupil=1.0-smoothstep(p.optical.w*0.9,p.optical.w*1.1,slope);
    light*=pupil;
    return RayResult(light,vec3<f32>(hits/8.0,tir/4.0,pupil));
}

@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) local_id:vec3<u32>) {
    let id=vec3<u32>(local_id.x,local_id.y+p.pad2,local_id.z);
    if(id.x>=p.output_width || id.y>=p.output_height){return;}
    let fieldaspect=f32(p.field_width)/f32(p.field_height);
    let outaspect=f32(p.output_width)/f32(p.output_height);
    let uv=(vec2<f32>(f32(id.x),f32(id.y))+vec2<f32>(0.5))/vec2<f32>(f32(p.output_width),f32(p.output_height));
    var q=vec2<f32>(uv.x*fieldaspect,uv.y);
    if(outaspect>fieldaspect){ q.y=(uv.y-0.5)*fieldaspect/outaspect+0.5; }
    else { q.x=(uv.x-0.5)*outaspect+fieldaspect*0.5; }
    // Five angular samples represent a finite illumination source. They are
    // deterministic, not random texture or a depth-independent image blur.
    let a=0.065;
    let center=trace_ray(q,vec2<f32>(0.0,0.0));
    if(p.pad1==1u){output[id.y*p.output_width+id.x]=pack4x8unorm(vec4<f32>(center.stats,1.0));return;}
    let light=(center.light*2.0+
      trace_ray(q,vec2<f32>(a,0.0)).light+trace_ray(q,vec2<f32>(-a,0.0)).light+
      trace_ray(q,vec2<f32>(0.0,a)).light+trace_ray(q,vec2<f32>(0.0,-a)).light)/6.0;
    let srgb=select(12.92*light,1.055*pow(max(light,vec3<f32>(0.0)),vec3<f32>(1.0/2.4))-0.055,light>vec3<f32>(0.0031308));
    output[id.y*p.output_width+id.x]=pack4x8unorm(vec4<f32>(clamp(srgb,vec3<f32>(0.0),vec3<f32>(1.0)),1.0));
}
