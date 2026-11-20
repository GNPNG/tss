use super::*;
use std::io::Write;
use std::path::Path;

fn fixture() -> LiveWheel {
    assert!(std::env::var_os("VK_DRIVER_FILES").is_some());
    let i = gpu::Instance::new(gpu::InstanceDescriptor {
        backends: gpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapters = i.enumerate_adapters(gpu::Backends::VULKAN);
    assert!(
        !adapters.is_empty()
            && adapters
                .iter()
                .all(|a| a.get_info().device_type == gpu::DeviceType::Cpu)
    );
    let a = &adapters[0];
    let (d, q) =
        pollster::block_on(a.request_device(&gpu::DeviceDescriptor::default(), None)).unwrap();
    let size = if std::env::var_os("TSS_TEST_WORLD_PROOF").is_some() {
        [640, 360]
    } else {
        [64, 36]
    };
    let c = gpu::SurfaceConfiguration {
        usage: gpu::TextureUsages::RENDER_ATTACHMENT,
        format: gpu::TextureFormat::Bgra8Unorm,
        width: size[0],
        height: size[1],
        present_mode: gpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: gpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
    };
    let bytes = if let Some(path) = std::env::var_os("TSS_TEST_FIELD_CACHE") {
        use std::io::Read;
        let file = std::fs::File::open(path).unwrap();
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(file)
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    } else {
        bundled_fields().unwrap()
    };
    pollster::block_on(LiveWheel::from_device(
        d,
        q,
        c,
        Fields::from_slice(&bytes).unwrap(),
        a.get_info().name,
        size,
    ))
    .unwrap()
}

fn map_readback(r: &LiveWheel) -> Vec<[u16; 4]> {
    let t = r
        .tracks
        .get(&r.active)
        .and_then(|t| t.warp.as_ref())
        .unwrap_or(&r.zero);
    read_rgba16(r, t, UV_SIZE)
}
fn read_rgba16(r: &LiveWheel, t: &Target, size: [u32; 2]) -> Vec<[u16; 4]> {
    let [w, h] = size;
    let row = w * 8;
    assert_eq!(row % gpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
    let b = r.device.create_buffer(&gpu::BufferDescriptor {
        label: Some("Diagnostic RGBA16F map"),
        size: u64::from(row * h),
        usage: gpu::BufferUsages::COPY_DST | gpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = r.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        t.texture.as_image_copy(),
        gpu::ImageCopyBuffer {
            buffer: &b,
            layout: gpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        extent(size, 1),
    );
    r.queue.submit(Some(enc.finish()));
    let sl = b.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    sl.map_async(gpu::MapMode::Read, move |s| tx.send(s).unwrap());
    r.wait_for_pending_work();
    rx.recv().unwrap().unwrap();
    let bytes = sl.get_mapped_range();
    bytes
        .chunks_exact(8)
        .map(|p| std::array::from_fn(|k| u16::from_le_bytes([p[2 * k], p[2 * k + 1]])))
        .collect()
}

fn half(h: u16) -> f64 {
    let sign = if h >> 15 == 0 { 1. } else { -1. };
    let exp = (h >> 10) & 31;
    let frac = f64::from(h & 1023);
    assert!(exp < 31);
    if exp == 0 {
        sign * frac * 2f64.powi(-24)
    } else {
        sign * (1. + frac / 1024.) * 2f64.powi(i32::from(exp) - 15)
    }
}
struct PoseSampler<'a> {
    lut: &'a [[f64; 4]],
    poses: [SlidePose; 2],
    rotation: [[f64; 2]; 2],
}
impl<'a> PoseSampler<'a> {
    fn new(lut: &'a [[f64; 4]], poses: [SlidePose; 2]) -> Self {
        Self {
            lut,
            poses,
            rotation: poses.map(|p| p.inverse_rotation().map(f64::from)),
        }
    }
    fn source(&self, p: [f64; 2], slide: usize) -> [f64; 2] {
        let aspect = 16. / 9.;
        let [c, s] = self.rotation[slide];
        let v = [(p[0] - 0.5) * aspect, p[1] - 0.5];
        let q = [
            0.5 + (c * v[0] - s * v[1]) / aspect,
            0.5 + s * v[0] + c * v[1],
        ];
        let center = self.poses[slide].center;
        let u = (q[0] - center[0]) * aspect / motion::TUG_SIDE + 0.5;
        let v = (q[1] - center[1]) / motion::TUG_SIDE + 0.5;
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return q;
        }
        let [w, h] = UV_SIZE;
        let x = (u * f64::from(w) - 0.5).clamp(0., f64::from(w - 1));
        let y = (v * f64::from(h) - 0.5).clamp(0., f64::from(h - 1));
        let (ix, iy) = (x as usize, y as usize);
        let jx = (ix + 1).min(w as usize - 1);
        let jy = (iy + 1).min(h as usize - 1);
        let k = slide * 2;
        std::array::from_fn(|i| {
            let t = |a: usize, b: usize| self.lut[b * w as usize + a][k + i];
            let tx = x - ix as f64;
            let ty = y - iy as f64;
            q[i] + (1. - ty) * ((1. - tx) * t(ix, iy) + tx * t(jx, iy))
                + ty * ((1. - tx) * t(ix, jy) + tx * t(jx, jy))
        })
    }
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn location(tick: usize) -> [f32; 2] {
    let a = tick as f64 * 2.35 / 40.;
    [
        (f64::floor((0.5 + 0.14 * a.cos()) * 2571.) / 2571.) as f32,
        (f64::floor((0.5 + 0.21 * a.sin()) * 1444.) / 1444.) as f32,
    ]
}

fn measure(
    tick: usize,
    map: &[[u16; 4]],
    base: &[[u16; 4]],
    poses: [SlidePose; 2],
    dir: Option<&Path>,
) {
    let [w, h] = UV_SIZE.map(|n| n as usize);
    assert!(
        map.iter().zip(base).all(|(a, b)| a[2..] == b[2..]),
        "Unaffected front mapping changed"
    );
    let lut: Vec<_> = map.iter().map(|p| p.map(half)).collect();
    let sampler = PoseSampler::new(&lut, poses);
    let mut points = Vec::with_capacity(w * h);
    let mut file = dir.map(|p| {
        std::io::BufWriter::new(
            std::fs::File::create_new(p.join(format!("rear-{tick:04}.rgf32"))).unwrap(),
        )
    });
    for y in 0..h {
        for x in 0..w {
            let uv = [(x as f64 + 0.5) / w as f64, (y as f64 + 0.5) / h as f64];
            let q = sampler.source(uv, 0);
            points.push([q[0] * w as f64, q[1] * h as f64]);
            if let Some(ref mut file) = file {
                for k in 0..2 {
                    file.write_all(&((q[k] - uv[k]) as f32).to_le_bytes())
                        .unwrap();
                }
            }
        }
    }
    let mut min: f64 = 1.;
    let mut max: f64 = 1.;
    let mut folds = 0;
    let mut ann_folds = 0;
    for y in 0..h - 1 {
        for x in 0..w - 1 {
            let i = y * w + x;
            let a = points[i];
            let b = points[i + 1];
            let c = points[i + w];
            let d = points[i + w + 1];
            for area in [cross(a, b, c), cross(b, d, c)] {
                min = min.min(area);
                max = max.max(area);
            }
            if cross(a, b, c).min(cross(b, d, c)) < 0. {
                folds += 1;
                let ellipse = ((((x + 1) as f64 / w as f64 - 0.5) / 0.14).powi(2)
                    + (((y + 1) as f64 / h as f64 - 0.5) / 0.21).powi(2))
                .sqrt();
                ann_folds += usize::from((0.7..=1.3).contains(&ellipse));
            }
        }
    }
    println!("{tick},0,{min:.9},{max:.9},{folds},{ann_folds}");
    assert_eq!(folds, 0);
    assert!(min > 0.98 && max < 1.02);
}
/// Replays the Windows benchmark ellipse at its measured 40-Hz cadence and reads
/// the actual production displacement texture. Runs only on an isolated CPU ICD.
#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn sustained_rear_swirl_on_software_vulkan() {
    let mut r = fixture();
    for (id, full) in [(11, true), (12, false)] {
        r.select_track(id);
        r.swirl(1, [0.44, 0.55], [0.58, 0.51]);
        if full {
            r.render_offscreen(1.25).unwrap();
        } else {
            assert!(r.apply_pending_swirl());
        }
        for i in 1..=4 {
            r.swirl(0, location(i - 1), location(i));
            if full {
                r.render_offscreen(1.25).unwrap();
            } else {
                assert!(r.apply_pending_swirl());
            }
        }
    }
    let accelerated = map_readback(&r);
    r.select_track(11);
    assert!(
        map_readback(&r) == accelerated,
        "Diagnostic preconditioning differs from normal input"
    );
    r.select_track(20);
    for i in 1..=16 {
        r.swirl(0, location(i - 1), location(i));
        r.apply_pending_swirl();
    }
    assert!(
        map_readback(&r).iter().all(|p| p[2..] == [0, 0]),
        "Unwarped front moved under rear drag"
    );
    r.select_track(21);
    r.swirl(1, [0.44, 0.55], [0.58, 0.51]);
    r.apply_pending_swirl();
    let base = map_readback(&r);
    let dir = std::env::var_os("TSS_WARP_DIAG_DIR").map(std::path::PathBuf::from);
    let milestones = [
        0, 1, 16, 60, 107, 214, 428, 560, 1000, 1284, 1679, 2079, 2080, 2081, 2082,
    ];
    println!(
        "strokes,front_changed_texels,min_signed_area,max_signed_area,folded_cells,annulus_folded"
    );
    measure(0, &base, &base, r.tracks[&21].slides, dir.as_deref());
    for i in 1..=2082 {
        let within = (i - 1) % 560;
        if within == 0 {
            r.end_gesture(0);
        }
        r.swirl(0, location(within), location(within + 1));
        assert!(r.apply_pending_swirl());
        if i % 16 == 0 {
            r.wait_for_pending_work();
        }
        if milestones.contains(&i) {
            measure(
                i,
                &map_readback(&r),
                &base,
                r.tracks[&21].slides,
                dir.as_deref(),
            );
        }
    }
}

const INTEGRATION_WGSL: &str = r#"
fn fractions(world:vec2<f32>,front:bool)->vec2<f32>{
 let rot=select(pair.rigid.xy,pair.rigid.zw,front);
 let origin=select(pair.centers.xy,pair.centers.zw,front);
 let q=sourceUV(world,rot,origin,front);
 if any(q<vec2<f32>(0.0))||any(q>=vec2<f32>(1.0)){return vec2<f32>(0.0);}
 let raw=textureSampleLevel(frames,smp,q,i32(pair.first),0.0);
 let oil=select(0.0,1.0,select(raw.r,raw.g,front)>=0.5);
 return vec2<f32>(oil,1.0-oil);
}
@fragment fn integrate(@location(0) uv:vec2<f32>)->@location(0) vec4<f32>{
 var total=vec4<f32>(0.0);
 for(var y=0u;y<4u;y++){for(var x=0u;x<4u;x++){
   let jitter=(vec2<f32>(f32(x),f32(y))+0.5)/4.0-0.5;
   let world=0.5+(uv+jitter/768.0-0.5)*vec2<f32>(2.25,4.0);
   total+=vec4<f32>(fractions(world,false),fractions(world,true));
 }}
 return total/16.0;
}
"#;
fn integrate(r: &mut LiveWheel) -> ([f64; 4], [f64; 4]) {
    r.render_offscreen(1.25).unwrap();
    let vert = shader(
        &r.device,
        "Native full-screen vertex",
        include_str!("vertex.wgsl"),
    );
    let source = include_str!("fields.wgsl").to_string() + INTEGRATION_WGSL;
    let frag = r.device.create_shader_module(gpu::ShaderModuleDescriptor {
        label: Some("Finite source inventory"),
        source: gpu::ShaderSource::Wgsl(source.into()),
    });
    use Binding::*;
    let image = Image(gpu::TextureViewDimension::D2);
    let pipe = pipeline(
        &r.device,
        &vert,
        &frag,
        "integrate",
        "Phase area integration",
        UV_FORMAT,
        &[
            Image(gpu::TextureViewDimension::D2Array),
            image,
            Sampler,
            Buffer,
        ],
    );
    let map = r
        .tracks
        .get(&r.active)
        .and_then(|t| t.warp.as_ref())
        .unwrap_or(&r.zero);
    use gpu::BindingResource::{Sampler as BoundSampler, TextureView};
    let group = bind(
        &r.device,
        &pipe,
        &[
            TextureView(&r.cache_view),
            TextureView(&map.view),
            BoundSampler(&r.sampler),
            r.pair_uniform.as_entire_binding(),
        ],
    );
    let size = [768, 768];
    let target = Target::new(&r.device, "Subpixel area", size, UV_FORMAT);
    let mut enc = r.device.create_command_encoder(&Default::default());
    draw(
        &mut enc,
        "Integrate oil and carrier",
        &target.view,
        &pipe,
        &group,
    );
    r.queue.submit(Some(enc.finish()));
    let data = read_rgba16(r, &target, size);
    let mut total = [0.; 4];
    let mut gate = [0.; 4];
    for (i, p) in data.into_iter().enumerate() {
        let u = ((i % 768) as f64 + 0.5) / 768.;
        let v = ((i / 768) as f64 + 0.5) / 768.;
        let inside = (u - 0.5).abs() <= 0.5 / 2.25 && (v - 0.5).abs() <= 0.5 / 4.;
        for c in 0..4 {
            let f = half(p[c]);
            total[c] += f;
            if inside {
                gate[c] += f;
            }
        }
    }
    (
        total.map(|n| n * 9. / (768. * 768.)),
        gate.map(|n| n * 9. / (768. * 768.)),
    )
}

#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn tug_rotation_preserve_both_finite_phase_inventories() {
    let mut r = fixture();
    let original = integrate(&mut r);
    println!(
        "cache {:?}, gate {:?}, frame 30. Entries are rear oil/carrier, front oil/carrier.",
        r.fields.size,
        r.fields.view_size()
    );
    println!("identity full={:?} gate={:?}", original.0, original.1);
    r.gesture(0, SlideGesture::Tug, [0.39, 0.44], [0.65, 0.60]);
    r.gesture(1, SlideGesture::Tug, [0.57, 0.67], [0.26, 0.44]);
    r.end_gesture(0);
    r.end_gesture(1);
    let tugged = integrate(&mut r);
    r.gesture(1, SlideGesture::Rotate, [0.75, 0.5], [0.5, 0.9]);
    r.end_gesture(1);
    assert!((r.tracks[&0].slides[1].angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
    assert_eq!(r.tracks[&0].slides[0].angle, 0.);
    let rotated = integrate(&mut r);
    for (name, (total, gate)) in [
        ("strong_pull_released", tugged),
        ("front_quarter_turn", rotated),
    ] {
        let delta: [f64; 4] = std::array::from_fn(|i| total[i] / original.0[i] - 1.);
        println!("{name} full={total:?} gate={gate:?} rel_delta={delta:?}");
        for f in delta {
            assert!(f.abs() < 0.0025, "Phase area error exceeds 0.25%");
        }
    }
    assert!(
        (rotated.1[2] - tugged.1[2]).abs() > 0.005,
        "Expected visible front inventory to change after quarter turn"
    );
    assert_eq!(
        rotated.0[0..2],
        tugged.0[0..2],
        "Rigid front changed rear full inventory"
    );
    assert_eq!(
        rotated.1[0..2],
        tugged.1[0..2],
        "Rigid front changed rear gate inventory"
    );
}

#[test]
#[ignore = "requires an isolated software Vulkan ICD"]
fn full_world_turn_brings_material_through_gate_and_returns_original_pixels() {
    let mut r = fixture();
    let dir = std::env::var_os("TSS_TEST_WORLD_PROOF").map(std::path::PathBuf::from);
    assert_eq!(r.fields.size, [704, 704]);
    let show = |r: &mut LiveWheel, label: &str| {
        r.render_offscreen(1.25).unwrap();
        let (w, h, rgba) = r.capture_rgba8().unwrap();
        if let Some(ref d) = dir {
            let mut image = std::io::BufWriter::new(
                std::fs::File::create_new(d.join(format!("{label}.ppm"))).unwrap(),
            );
            write!(image, "P6\n{w} {h}\n255\n").unwrap();
            for p in rgba.chunks_exact(4) {
                image.write_all(&p[..3]).unwrap();
            }
        }
        rgba
    };
    let before = show(&mut r, "gate-before");
    r.gesture(1, SlideGesture::Rotate, [0.75, 0.5], [0.5, 0.9]);
    r.end_gesture(1);
    let turned = show(&mut r, "gate-quarter");
    assert!(turned != before);
    r.gesture(1, SlideGesture::Rotate, [0.5, 0.9], [0.75, 0.5]);
    r.end_gesture(1);
    assert!(
        before == show(&mut r, "gate-return"),
        "Returning the front slide lost material or moved the rear"
    );
}
