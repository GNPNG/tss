//! Live presentation of the two independently simulated Basilisk slide fields.
mod feedback;
mod projector;
pub use projector::{ProjectorOutput, ProjectorTiming};
mod fields;
pub use feedback::FeedbackSettings;
mod movie;
pub use movie::WheelMovie;
mod material;
mod motion;
mod preset;
mod pressure;
mod small_front;
use small_front::{cache_slots, upload_frame, SmallFront};
#[cfg(test)]
mod coral_layer_proof;
pub use fields::bundled_fields;
pub use motion::SlideGesture;
use motion::SlidePose;
pub use preset::MotionPreset;
pub use wgpu;

use bytemuck::{Pod, Zeroable};
use fields::Fields;
use material::Material;
use std::collections::HashMap;
use wgpu::{self as gpu, util::DeviceExt};

const UV_SIZE: [u32; 2] = [320, 180];
const FIELD_FORMAT: gpu::TextureFormat = gpu::TextureFormat::Rgba16Float;
const UV_FORMAT: gpu::TextureFormat = gpu::TextureFormat::Rgba16Float;
const COLOR_FORMAT: gpu::TextureFormat = gpu::TextureFormat::Rgba8Unorm;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Brush {
    params: [[f32; 4]; 2],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FramePair {
    first: u32,
    second: u32,
    fraction: f32,
    front_fraction: f32,
    rotation: [f32; 4],
    centers: [f32; 4],
    cache_rect: [f32; 4],
    pressure: [[f32; 4]; 2],
    front_frames: [u32; 4],
}

struct Target {
    texture: gpu::Texture,
    view: gpu::TextureView,
}
impl Target {
    fn new(d: &gpu::Device, label: &str, size: [u32; 2], fmt: gpu::TextureFormat) -> Self {
        let texture = d.create_texture(&gpu::TextureDescriptor {
            label: Some(label),
            size: extent(size, 1),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: fmt,
            usage: gpu::TextureUsages::TEXTURE_BINDING
                | gpu::TextureUsages::RENDER_ATTACHMENT
                | gpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self { texture, view }
    }
}
fn extent([width, height]: [u32; 2], depth_or_array_layers: u32) -> gpu::Extent3d {
    gpu::Extent3d {
        width,
        height,
        depth_or_array_layers,
    }
}
#[derive(Default)]
struct Track {
    warp: Option<Target>,
    slides: [SlidePose; 2],
    dirty: bool,
}
struct Pass {
    pipeline: gpu::RenderPipeline,
    layout: gpu::BindGroupLayout,
}
#[derive(Clone, Copy)]
enum Binding {
    Image(gpu::TextureViewDimension),
    Sampler,
    Buffer,
}
fn pipeline(
    d: &gpu::Device,
    vertex: &gpu::ShaderModule,
    fragment: &gpu::ShaderModule,
    entry: &str,
    label: &str,
    target: impl Into<gpu::ColorTargetState>,
    bindings: &[Binding],
) -> Pass {
    let entries: Vec<_> = bindings
        .iter()
        .enumerate()
        .map(|(i, b)| gpu::BindGroupLayoutEntry {
            binding: i as u32,
            visibility: gpu::ShaderStages::FRAGMENT,
            count: None,
            ty: match b {
                Binding::Image(dim) => gpu::BindingType::Texture {
                    sample_type: gpu::TextureSampleType::Float { filterable: true },
                    view_dimension: *dim,
                    multisampled: false,
                },
                Binding::Sampler => gpu::BindingType::Sampler(gpu::SamplerBindingType::Filtering),
                Binding::Buffer => gpu::BindingType::Buffer {
                    ty: gpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            },
        })
        .collect();
    let layout = d.create_bind_group_layout(&gpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    });
    let pipe_layout = d.create_pipeline_layout(&gpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let pipeline = d.create_render_pipeline(&gpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipe_layout),
        vertex: gpu::VertexState {
            module: vertex,
            entry_point: "main",
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(gpu::FragmentState {
            module: fragment,
            entry_point: entry,
            targets: &[Some(target.into())],
        }),
        multiview: None,
    });
    Pass { pipeline, layout }
}
fn bind(d: &gpu::Device, pass: &Pass, resources: &[gpu::BindingResource]) -> gpu::BindGroup {
    let e: Vec<_> = resources
        .iter()
        .enumerate()
        .map(|(i, r)| gpu::BindGroupEntry {
            binding: i as u32,
            resource: r.clone(),
        })
        .collect();
    d.create_bind_group(&gpu::BindGroupDescriptor {
        label: None,
        layout: &pass.layout,
        entries: &e,
    })
}
fn draw(
    enc: &mut gpu::CommandEncoder,
    label: &str,
    dst: &gpu::TextureView,
    pipe: &Pass,
    group: &gpu::BindGroup,
) {
    let mut p = enc.begin_render_pass(&gpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(gpu::RenderPassColorAttachment {
            view: dst,
            resolve_target: None,
            ops: gpu::Operations {
                load: gpu::LoadOp::Clear(gpu::Color::TRANSPARENT),
                store: gpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    p.set_pipeline(&pipe.pipeline);
    p.set_bind_group(0, group, &[]);
    p.draw(0..3, 0..1);
}
fn uniform<T: Pod>(d: &gpu::Device, label: &str, value: &T) -> gpu::Buffer {
    d.create_buffer_init(&gpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(value),
        usage: gpu::BufferUsages::UNIFORM | gpu::BufferUsages::COPY_DST,
    })
}
fn shader(d: &gpu::Device, label: &str, code: &'static str) -> gpu::ShaderModule {
    d.create_shader_module(gpu::ShaderModuleDescriptor {
        label: Some(label),
        source: gpu::ShaderSource::Wgsl(code.into()),
    })
}
fn preview_size(cfg: &gpu::SurfaceConfiguration, cap: u32) -> [u32; 2] {
    let width = cfg.width.min(cap).max(1);
    // A slide has fixed 16:9 coordinates even when its presentation viewport resizes.
    [width, (width * 9 / 16).max(1)]
}
fn check_extent(size: [u32; 2], max_dimension: u32) -> Result<(), String> {
    if size.iter().any(|&n| n > max_dimension) {
        return Err(format!(
            "Requested {} × {} exceeds the adapter's {max_dimension}-pixel texture limit",
            size[0], size[1]
        ));
    }
    Ok(())
}

pub struct LiveWheel {
    device: gpu::Device,
    queue: gpu::Queue,
    adapter: String,
    surface_adapter: Option<gpu::Adapter>,
    cfg: gpu::SurfaceConfiguration,
    fields: Fields,
    motion: MotionPreset,
    inactive_fields: Option<Fields>,
    small_front: Option<SmallFront>,
    cache: gpu::Texture,
    cache_view: gpu::TextureView,
    slots: [Option<u32>; 2],
    tracks: HashMap<u64, Track>,
    active: u64,
    zero: Target,
    interpolated: Target,
    sampler: gpu::Sampler,
    brush_uniform: gpu::Buffer,
    pair_uniform: gpu::Buffer,
    material_uniform: gpu::Buffer,
    materials: HashMap<String, Material>,
    wheel: String,
    focus: Option<f32>,
    slide_visibility: [bool; 2],
    field_passes: [Pass; 4],
    warp_pass: Pass,
    baseline_pass: Pass,
    palette_pass: Pass,
    footprint_baseline_pass: Pass,
    footprint_palette_pass: Pass,
    material_pass: Pass,
    film_material: Option<Target>,
    footprint_sampling: bool,
    feedback: Option<feedback::Feedback>,
    blit_pass: Pass,
    optics: Target,
    cap: u32,
    actual_size: [u32; 2],
}
impl LiveWheel {
    pub async fn new(
        instance: &gpu::Instance,
        surface: &gpu::Surface<'_>,
        bytes: &[u8],
        width: u32,
        height: u32,
        preferred_adapter: Option<&str>,
    ) -> Result<Self, String> {
        let fields = Fields::from_slice(bytes)?;
        let mut adapters = instance.enumerate_adapters(gpu::Backends::all());
        adapters.retain(|a| {
            let i = a.get_info();
            a.is_surface_supported(surface)
                && matches!(
                    i.device_type,
                    gpu::DeviceType::DiscreteGpu | gpu::DeviceType::IntegratedGpu
                )
                && preferred_adapter.map_or(true, |name| {
                    i.name.to_lowercase().contains(&name.to_lowercase())
                })
        });
        adapters.sort_by_key(|a| {
            (
                a.get_info().device_type != gpu::DeviceType::DiscreteGpu,
                a.get_info().backend != gpu::Backend::Vulkan,
            )
        });
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or("No matching hardware GPU can present to the editor window")?;
        let info = adapter.get_info();
        let adapter_name = format!("{} ({:?})", info.name, info.backend);
        let limits = gpu::Limits::downlevel_defaults()
            .using_resolution(adapter.limits())
            .using_alignment(adapter.limits());
        check_extent([width, height], limits.max_texture_dimension_2d)?;
        check_extent(fields.size, limits.max_texture_dimension_2d)?;
        let (device, queue) = adapter
            .request_device(
                &gpu::DeviceDescriptor {
                    label: Some("TSS live slide renderer"),
                    required_features: gpu::Features::empty(),
                    required_limits: limits,
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut cfg = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or("No compatible swap chain")?;
        let formats = surface.get_capabilities(&adapter).formats;
        if let Some(fmt) = formats.into_iter().find(|f| {
            matches!(
                f,
                gpu::TextureFormat::Bgra8Unorm | gpu::TextureFormat::Rgba8Unorm
            )
        }) {
            cfg.format = fmt;
        }
        cfg.present_mode = gpu::PresentMode::Fifo;
        cfg.desired_maximum_frame_latency = 2;
        if width > 0 && height > 0 {
            surface.configure(&device, &cfg);
        }
        let mut renderer =
            Self::from_device(device, queue, cfg, fields, adapter_name, [width, height]).await?;
        renderer.surface_adapter = Some(adapter);
        Ok(renderer)
    }
    async fn from_device(
        device: gpu::Device,
        queue: gpu::Queue,
        cfg: gpu::SurfaceConfiguration,
        fields: Fields,
        adapter_name: String,
        actual_size: [u32; 2],
    ) -> Result<Self, String> {
        device.push_error_scope(gpu::ErrorFilter::Validation);
        let v = shader(&device, "Fullscreen triangle", include_str!("vertex.wgsl"));
        let wf = shader(
            &device,
            "Per-slide gesture mapping",
            include_str!("warp.wgsl"),
        );
        let interp = shader(
            &device,
            "Basilisk frame interpolation",
            include_str!("fields.wgsl"),
        );
        let blit = shader(&device, "Present native preview", include_str!("blit.wgsl"));
        let optics_base = device.create_shader_module(gpu::ShaderModuleDescriptor {
            label: Some("Shared Violet / Blue optics"),
            source: gpu::util::make_spirv(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/violet-blue.spv"
            ))),
        });
        let optics_palette = device.create_shader_module(gpu::ShaderModuleDescriptor {
            label: Some("Shared fitted wheel optics"),
            source: gpu::util::make_spirv(include_bytes!(concat!(env!("OUT_DIR"), "/wheel.spv"))),
        });
        use Binding::*;
        let image = Image(gpu::TextureViewDimension::D2);
        let warp_pass = pipeline(
            &device,
            &v,
            &wf,
            "main",
            "Material pull",
            UV_FORMAT,
            &[Buffer],
        );
        // RG hold rear/front distance codes (zero = clear carrier), BA their
        // respective footprints. Unwritten channels retain the pass clear value.
        use gpu::ColorWrites;
        let field_passes = [
            ("Clear carrier", ColorWrites::empty()),
            ("Rear film", ColorWrites::RED | ColorWrites::BLUE),
            ("Front film", ColorWrites::GREEN | ColorWrites::ALPHA),
            ("Buffered slide states", ColorWrites::ALL),
        ]
        .map(|(label, write_mask)| {
            pipeline(
                &device,
                &v,
                &interp,
                "main",
                label,
                gpu::ColorTargetState {
                    format: FIELD_FORMAT,
                    blend: None,
                    write_mask,
                },
                &[
                    Image(gpu::TextureViewDimension::D2Array),
                    image,
                    Sampler,
                    Buffer,
                ],
            )
        });
        let baseline_pass = pipeline(
            &device,
            &v,
            &optics_base,
            "main",
            "Violet / Blue",
            COLOR_FORMAT,
            &[image, Sampler, Buffer],
        );
        let palette_pass = pipeline(
            &device,
            &v,
            &optics_palette,
            "main",
            "Mathmos wheel",
            COLOR_FORMAT,
            &[image, Sampler, Buffer],
        );
        let footprint_bindings = [
            image,
            Sampler,
            Buffer,
            Image(gpu::TextureViewDimension::D2Array),
            image,
            Buffer,
            image,
        ];
        let footprint_pass = |label: &str, bytes: &[u8], format, bindings: &[Binding]| {
            let fragment = device.create_shader_module(gpu::ShaderModuleDescriptor {
                label: Some(label),
                source: gpu::util::make_spirv(bytes),
            });
            pipeline(&device, &v, &fragment, "main", label, format, bindings)
        };
        let footprint_baseline_pass = footprint_pass(
            "Violet / Blue finite pixel",
            include_bytes!(concat!(env!("OUT_DIR"), "/violet-blue-footprint.spv")),
            COLOR_FORMAT,
            &footprint_bindings,
        );
        let footprint_palette_pass = footprint_pass(
            "Wheel finite pixel",
            include_bytes!(concat!(env!("OUT_DIR"), "/wheel-footprint.spv")),
            COLOR_FORMAT,
            &footprint_bindings,
        );
        let material_pass = footprint_pass(
            "Slide optical properties",
            include_bytes!(concat!(env!("OUT_DIR"), "/film-material.spv")),
            FIELD_FORMAT,
            &footprint_bindings[..6],
        );
        let blit_entry = if cfg.format.is_srgb() {
            "decode_for_srgb"
        } else {
            "copy_encoded"
        };
        let blit_pass = pipeline(
            &device,
            &v,
            &blit,
            blit_entry,
            "Presentation",
            cfg.format,
            &[image, Sampler],
        );
        let cache = device.create_texture(&gpu::TextureDescriptor {
            label: Some("Adjacent film frames for independent slide clocks"),
            size: extent(fields.size, 4),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::Rg8Unorm,
            usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let cache_view = cache.create_view(&gpu::TextureViewDescriptor {
            dimension: Some(gpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let interpolated = Target::new(
            &device,
            "Interpolated and displaced distances",
            fields.view_size(),
            FIELD_FORMAT,
        );
        let zero = Target::new(&device, "Undisturbed slide mappings", UV_SIZE, UV_FORMAT);
        let optics = Target::new(
            &device,
            "Reduced-resolution optical projection",
            preview_size(&cfg, 640),
            COLOR_FORMAT,
        );
        let sampler = device.create_sampler(&gpu::SamplerDescriptor {
            label: Some("Slide bilinear filtering"),
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        let brush_uniform = uniform(&device, "Pointer brush segments", &Brush::zeroed());
        let pair_uniform = uniform(&device, "Temporal fraction", &FramePair::zeroed());
        let materials = material::load()?;
        let material_uniform = uniform(
            &device,
            "Projection material",
            materials
                .get("violet-blue")
                .ok_or("Missing Violet / Blue")?,
        );
        if let Some(error) = device.pop_error_scope().await {
            return Err(error.to_string());
        }
        Ok(Self {
            device,
            queue,
            adapter: adapter_name,
            surface_adapter: None,
            cfg,
            fields,
            motion: MotionPreset::WheelFlow,
            inactive_fields: None,
            small_front: None,
            cache,
            cache_view,
            slots: [None, None],
            tracks: HashMap::new(),
            active: 0,
            zero,
            interpolated,
            sampler,
            brush_uniform,
            pair_uniform,
            material_uniform,
            materials,
            wheel: "violet-blue".into(),
            focus: None,
            slide_visibility: [true; 2],
            field_passes,
            warp_pass,
            baseline_pass,
            palette_pass,
            footprint_baseline_pass,
            footprint_palette_pass,
            material_pass,
            film_material: None,
            footprint_sampling: false,
            feedback: None,
            blit_pass,
            optics,
            cap: 640,
            actual_size,
        })
    }
    pub fn adapter_name(&self) -> &str {
        &self.adapter
    }
    pub fn field_size(&self) -> [u32; 2] {
        self.fields.view_size()
    }
    pub fn preview_size(&self) -> [u32; 2] {
        [self.optics.texture.width(), self.optics.texture.height()]
    }
    pub fn set_preview_width(&mut self, width: u32) -> Result<(), String> {
        if !(160..=1280).contains(&width) {
            return Err("Preview width must be between 160 and 1280".into());
        }
        self.cap = width;
        self.update_target();
        Ok(())
    }
    fn update_target(&mut self) {
        let size = preview_size(&self.cfg, self.cap);
        if size != [self.optics.texture.width(), self.optics.texture.height()] {
            self.optics = Target::new(&self.device, "Optical projection", size, COLOR_FORMAT);
        }
    }
    pub fn resize(
        &mut self,
        surface: &gpu::Surface<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if self.actual_size == [width, height] {
            return Ok(());
        }
        check_extent(
            [width, height],
            self.device.limits().max_texture_dimension_2d,
        )?;
        self.actual_size = [width, height];
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.cfg.width = width;
        self.cfg.height = height;
        surface.configure(&self.device, &self.cfg);
        self.update_target();
        Ok(())
    }
    pub fn select_track(&mut self, id: u64) {
        self.active = id;
    }
    /// Enable the finite-pixel optical path on deformed tracks for validation.
    pub fn set_footprint_sampling(&mut self, enabled: bool) {
        self.footprint_sampling = enabled;
    }
    /// Expose either film to the projector while both clocks and poses keep advancing.
    pub fn set_slide_visibility(&mut self, rear: bool, front: bool) {
        if self.slide_visibility != [rear, front] {
            self.slide_visibility = [rear, front];
            self.reset_feedback_history();
        }
    }
    pub fn forget_track(&mut self, id: u64) {
        self.tracks.remove(&id);
    }
    pub fn set_wheel(&mut self, id: &str) -> Result<(), String> {
        if !self.materials.contains_key(id) {
            return Err(format!("Unknown Mathmos wheel {id}"));
        }
        self.wheel = id.into();
        self.upload_material();
        Ok(())
    }
    /// Switch film geometry while retaining the optics, poses and GPU allocation.
    pub fn set_motion(&mut self, preset: MotionPreset) -> Result<(), String> {
        if self.motion == preset {
            return Ok(());
        }
        if preset == MotionPreset::WheelWithCoralBeads && self.small_front.is_none() {
            if !self.fields.matches_small_front() {
                return Err(
                    "Small coral droplets require the registered full-slide geometry".into(),
                );
            }
            self.small_front = Some(SmallFront::load()?);
        }
        if self.motion == MotionPreset::CoralRelease || preset == MotionPreset::CoralRelease {
            if self.inactive_fields.is_none() {
                let coral = Fields::coral_release()?;
                if !coral.same_glass_as(&self.fields) {
                    return Err("Coral release requires the registered full-slide geometry".into());
                }
                self.inactive_fields = Some(coral);
            }
            std::mem::swap(&mut self.fields, self.inactive_fields.as_mut().unwrap());
            self.slots = [None, None];
        }
        self.motion = preset;
        self.reset_feedback_history();
        Ok(())
    }
    fn upload_material(&self) {
        let mut material = self.materials[&self.wheel];
        material.lens[0] = self.focus.unwrap_or(-1.0);
        material.lens[1] = if self.feedback.is_some() { 1.0 } else { 0.0 };
        self.queue
            .write_buffer(&self.material_uniform, 0, bytemuck::bytes_of(&material));
    }
    pub fn set_feedback(&mut self, settings: Option<FeedbackSettings>) -> Result<(), String> {
        if let Some(settings) = settings {
            settings.validate()?;
            if let Some(f) = &mut self.feedback {
                f.settings = settings;
            } else {
                self.feedback = Some(feedback::Feedback::new(
                    &self.device,
                    self.preview_size(),
                    settings,
                ));
            }
        } else {
            self.feedback = None;
        }
        self.upload_material();
        Ok(())
    }
    pub fn reset_feedback_history(&mut self) {
        if let Some(f) = &mut self.feedback {
            f.reset();
        }
    }
    pub fn set_focus(&mut self, focus: Option<f32>) -> Result<(), String> {
        if focus.is_some_and(|f| !(0.0..=1.0).contains(&f)) {
            return Err("Focus must lie between the rear and front slide planes".into());
        }
        self.focus = focus;
        self.upload_material();
        Ok(())
    }
    pub fn swirl(&mut self, slide: u32, from: [f32; 2], to: [f32; 2]) {
        self.gesture(slide, SlideGesture::Tug, from, to);
    }
    pub fn gesture(&mut self, slide: u32, mode: SlideGesture, from: [f32; 2], to: [f32; 2]) {
        if slide > 1 || from == to || from.into_iter().chain(to).any(|v| !v.is_finite()) {
            return;
        }
        let t = self.tracks.entry(self.active).or_default();
        let bounded = |v: f32| v.clamp(0., 1.);
        t.dirty |= t.slides[slide as usize].gesture(mode, from.map(bounded), to.map(bounded));
    }
    pub fn set_tug_response(&mut self, slide: u32, response: f32) -> Result<(), String> {
        if slide > 1 || !(0.0..=1.).contains(&response) {
            return Err("Expected slide 0 or 1 and response in [0, 1]".into());
        }
        self.tracks.entry(self.active).or_default().slides[slide as usize].response = response;
        Ok(())
    }
    /// Set one sealed slide's normalized contact load at a top-left-origin gate position.
    /// The contact remains in glass coordinates when the slide subsequently rotates.
    pub fn set_slide_pressure(
        &mut self,
        slide: u32,
        amount: f32,
        center: [f32; 2],
    ) -> Result<(), String> {
        if slide > 1
            || !(0. ..=1.).contains(&amount)
            || center.iter().any(|v| !(0. ..=1.).contains(v))
        {
            return Err("Expected slide 0 or 1, pressure and gate coordinates in [0, 1]".into());
        }
        let pose = &mut self.tracks.entry(self.active).or_default().slides[slide as usize];
        pose.pressure = pressure::Pressure::new(
            pose.rigid_point(center.map(f64::from)),
            amount,
            self.fields.view_to_cache(),
        );
        Ok(())
    }
    pub fn end_gesture(&mut self, slide: u32) {
        if let Some(pose) = self
            .tracks
            .get_mut(&self.active)
            .and_then(|t| t.slides.get_mut(slide as usize))
        {
            pose.end();
        }
    }
    fn cache_pair(&mut self, a: u32, b: u32) -> [u32; 2] {
        cache_slots(&mut self.slots, [a, b], |slot, frame| {
            upload_frame(
                &self.queue,
                &self.cache,
                self.fields.size,
                slot,
                self.fields.frame(frame),
            );
        })
    }
    pub fn render(
        &mut self,
        surface: &gpu::Surface<'_>,
        time_seconds: f64,
    ) -> Result<bool, String> {
        self.render_with_program_time(surface, time_seconds, time_seconds)
    }
    pub fn render_with_program_time(
        &mut self,
        surface: &gpu::Surface<'_>,
        time_seconds: f64,
        program_time: f64,
    ) -> Result<bool, String> {
        self.render_film_times(surface, time_seconds, time_seconds, program_time)
    }
    pub fn render_film_times(
        &mut self,
        surface: &gpu::Surface<'_>,
        rear: f64,
        front: f64,
        program_time: f64,
    ) -> Result<bool, String> {
        if self.actual_size.contains(&0) {
            return Ok(false);
        }
        if !rear.is_finite() || !front.is_finite() || !program_time.is_finite() {
            return Err("Invalid live frame time".into());
        }
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(gpu::SurfaceError::Timeout) => return Ok(false),
            Err(gpu::SurfaceError::Lost | gpu::SurfaceError::Outdated) => {
                surface.configure(&self.device, &self.cfg);
                return Ok(false);
            }
            Err(e) => return Err(e.to_string()),
        };
        let out = frame.texture.create_view(&Default::default());
        self.render_films_to(Some(&out), rear, front, program_time);
        frame.present();
        Ok(true)
    }
    /// Render the current track to the preview texture without acquiring a surface.
    /// The resulting frame is available through `capture_rgba8`.
    pub fn render_program_offscreen(
        &mut self,
        time_seconds: f64,
        program_time: f64,
    ) -> Result<(), String> {
        self.render_film_times_offscreen(time_seconds, time_seconds, program_time)
    }
    pub fn render_film_times_offscreen(
        &mut self,
        rear: f64,
        front: f64,
        program_time: f64,
    ) -> Result<(), String> {
        if !rear.is_finite() || !front.is_finite() || !program_time.is_finite() {
            return Err("Invalid live frame time".into());
        }
        self.render_films_to(None, rear, front, program_time);
        Ok(())
    }
    pub fn render_offscreen(&mut self, time_seconds: f64) -> Result<(), String> {
        if !time_seconds.is_finite() {
            return Err("Invalid live frame time".into());
        }
        self.render_to(None, time_seconds);
        Ok(())
    }

    /// Apply the queued input to the selected track without field, optical, or presentation work.
    /// Diagnostic preconditioning calls this once per recorded input batch.
    pub fn apply_pending_swirl(&mut self) -> bool {
        if !self
            .tracks
            .get(&self.active)
            .is_some_and(|track| track.dirty)
        {
            return false;
        }
        let mut enc = self
            .device
            .create_command_encoder(&gpu::CommandEncoderDescriptor {
                label: Some("Diagnostic swirl preconditioning"),
            });
        self.encode_pending_swirl(&mut enc);
        self.queue.submit(Some(enc.finish()));
        true
    }

    /// Wait for submitted diagnostic work before enqueueing another batch.
    pub fn wait_for_pending_work(&self) {
        self.device.poll(gpu::Maintain::Wait);
    }

    fn encode_pending_swirl(&mut self, enc: &mut gpu::CommandEncoder) {
        let Some(t) = self.tracks.get_mut(&self.active) else {
            return;
        };
        if !t.dirty {
            return;
        }
        let brush = Brush {
            params: t
                .slides
                .map(|s| [s.strength[0] as f32, s.strength[1] as f32, 0., 0.]),
        };
        self.queue
            .write_buffer(&self.brush_uniform, 0, bytemuck::bytes_of(&brush));
        let dest = t.warp.get_or_insert_with(|| {
            Target::new(
                &self.device,
                "Independent compact material displacements",
                UV_SIZE,
                UV_FORMAT,
            )
        });
        let group = bind(
            &self.device,
            &self.warp_pass,
            &[self.brush_uniform.as_entire_binding()],
        );
        draw(
            enc,
            "Evaluate material pull",
            &dest.view,
            &self.warp_pass,
            &group,
        );
        t.dirty = false;
    }
    fn render_to(&mut self, out: Option<&gpu::TextureView>, time_seconds: f64) {
        self.render_program_to(out, time_seconds, time_seconds);
    }
    fn render_program_to(
        &mut self,
        out: Option<&gpu::TextureView>,
        time_seconds: f64,
        program_time: f64,
    ) {
        self.render_films_to(out, time_seconds, time_seconds, program_time);
    }
    fn render_films_to(
        &mut self,
        out: Option<&gpu::TextureView>,
        rear_time: f64,
        front_time: f64,
        program_time: f64,
    ) {
        let size = self.preview_size();
        if let Some(f) = &mut self.feedback {
            f.prepare(&self.device, size);
        }
        let (a, b, fraction) = self.fields.sample_motion(self.motion, rear_time);
        let [first, second] = self.cache_pair(a, b);
        let (front_frames, front_fraction) = if self.motion == MotionPreset::WheelWithCoralBeads {
            let (a, b, alpha) = self
                .fields
                .sample_motion(MotionPreset::CoralRelease, front_time);
            (
                self.small_front
                    .as_mut()
                    .expect("loaded by set_motion")
                    .frames(&self.queue, &self.cache, a, b),
                alpha,
            )
        } else {
            ([first, second], fraction)
        };
        let mut poses = self
            .tracks
            .get(&self.active)
            .map(|t| t.slides)
            .unwrap_or_default();
        // The finite-pixel pass uses rotation and pressure to choose the pupil
        // footprint. Keep a hidden film's deformation out of that decision.
        for (pose, visible) in poses.iter_mut().zip(self.slide_visibility) {
            if !visible {
                *pose = SlidePose::default();
            }
        }
        let [r, f] = poses.map(|p| p.inverse_rotation());
        let [rc, fc] = poses.map(|p| p.center.map(|v| v as f32));
        self.queue.write_buffer(
            &self.pair_uniform,
            0,
            bytemuck::bytes_of(&FramePair {
                first,
                second,
                fraction,
                front_fraction,
                front_frames: [front_frames[0], front_frames[1], 0, 0],
                rotation: [r[0], r[1], f[0], f[1]],
                centers: [rc[0], rc[1], fc[0], fc[1]],
                cache_rect: self.fields.view_to_cache(),
                pressure: poses.map(|p| p.pressure.shader(p.angle)),
            }),
        );
        let mut enc = self
            .device
            .create_command_encoder(&gpu::CommandEncoderDescriptor {
                label: Some("TSS live frame"),
            });
        use gpu::BindingResource::{Sampler, TextureView};
        self.encode_pending_swirl(&mut enc);
        let mapping = self
            .tracks
            .get(&self.active)
            .and_then(|t| t.warp.as_ref())
            .unwrap_or(&self.zero);
        let [rear, front] = self.slide_visibility;
        let field_pass = &self.field_passes[usize::from(rear) + 2 * usize::from(front)];
        let fields = bind(
            &self.device,
            field_pass,
            &[
                TextureView(&self.cache_view),
                TextureView(&mapping.view),
                Sampler(&self.sampler),
                self.pair_uniform.as_entire_binding(),
            ],
        );
        draw(
            &mut enc,
            "Reconstruct moving independent slides",
            &self.interpolated.view,
            field_pass,
            &fields,
        );
        // Keep the selected optical model continuous through first movement.
        let integrate = self.footprint_sampling || poses.iter().any(|p| p.pressure.active());
        let optic_pipe = match (integrate, self.wheel == "violet-blue") {
            (true, true) => &self.footprint_baseline_pass,
            (true, false) => &self.footprint_palette_pass,
            (false, true) => &self.baseline_pass,
            (false, false) => &self.palette_pass,
        };
        if integrate && self.film_material.is_none() {
            let [width, height] = self.fields.view_size();
            let limit = self.device.limits().max_texture_dimension_2d;
            self.film_material = Some(Target::new(
                &self.device,
                "Rear and front optical properties",
                [width, height.min(limit / 2) * 2],
                FIELD_FORMAT,
            ));
        }
        let material_target = self.film_material.as_ref().unwrap_or(&self.zero);
        let optical_resources = [
            TextureView(&self.interpolated.view),
            Sampler(&self.sampler),
            self.material_uniform.as_entire_binding(),
            TextureView(&self.cache_view),
            TextureView(&mapping.view),
            self.pair_uniform.as_entire_binding(),
            TextureView(&material_target.view),
        ];
        let entries = if integrate {
            let properties = bind(&self.device, &self.material_pass, &optical_resources[..6]);
            draw(
                &mut enc,
                "Build both slide optical properties",
                &material_target.view,
                &self.material_pass,
                &properties,
            );
            &optical_resources[..]
        } else {
            &optical_resources[..3]
        };
        let (optic_pipe, destination) = if let Some(f) = &self.feedback {
            f.optics(integrate, self.wheel == "violet-blue")
        } else {
            (optic_pipe, &self.optics.view)
        };
        let optics = bind(&self.device, optic_pipe, entries);
        draw(
            &mut enc,
            "Transmitted projection",
            destination,
            optic_pipe,
            &optics,
        );
        if let Some(f) = &mut self.feedback {
            f.compose(
                &self.device,
                &self.queue,
                &mut enc,
                &self.sampler,
                &self.optics.view,
                program_time,
            );
        }
        if let Some(out) = out {
            let blit = bind(
                &self.device,
                &self.blit_pass,
                &[TextureView(&self.optics.view), Sampler(&self.sampler)],
            );
            draw(&mut enc, "Present preview", out, &self.blit_pass, &blit);
        }
        self.queue.submit(Some(enc.finish()));
    }
    pub fn capture_rgba8(&self) -> Result<(u32, u32, Vec<u8>), String> {
        let [w, h] = self.preview_size();
        let row =
            (w * 4).div_ceil(gpu::COPY_BYTES_PER_ROW_ALIGNMENT) * gpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&gpu::BufferDescriptor {
            label: Some("GPU capture outside benchmark"),
            size: (row * h) as u64,
            usage: gpu::BufferUsages::COPY_DST | gpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut e = self.device.create_command_encoder(&Default::default());
        e.copy_texture_to_buffer(
            self.optics.texture.as_image_copy(),
            gpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: gpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            extent([w, h], 1),
        );
        self.queue.submit(Some(e.finish()));
        let slice = buffer.slice(..);
        let (send, recv) = std::sync::mpsc::channel();
        slice.map_async(gpu::MapMode::Read, move |r| {
            let _ = send.send(r);
        });
        self.device.poll(gpu::Maintain::Wait);
        recv.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let memory = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for r in memory.chunks_exact(row as usize) {
            pixels.extend_from_slice(&r[..(w * 4) as usize]);
        }
        drop(memory);
        buffer.unmap();
        Ok((w, h, pixels))
    }
}

#[cfg(test)]
mod coral_capture;
#[cfg(test)]
mod feedback_test;
#[cfg(test)]
mod footprint_test;
#[cfg(test)]
mod pressure_test;
#[cfg(test)]
mod software_test;
#[cfg(test)]
mod warp_diagnostic;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_shaders_validate_without_a_gpu() {
        use wgpu::naga::{front, valid};
        let validate = |m| {
            valid::Validator::new(valid::ValidationFlags::all(), valid::Capabilities::all())
                .validate(&m)
                .unwrap();
        };
        for (name, s) in [
            ("vertex", include_str!("vertex.wgsl")),
            ("warp", include_str!("warp.wgsl")),
            ("fields", include_str!("fields.wgsl")),
            ("blit", include_str!("blit.wgsl")),
        ] {
            validate(
                front::wgsl::parse_str(s)
                    .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(s))),
            );
        }
        for b in [
            include_bytes!(concat!(env!("OUT_DIR"), "/violet-blue.spv")).as_slice(),
            include_bytes!(concat!(env!("OUT_DIR"), "/wheel.spv")).as_slice(),
        ] {
            validate(front::spv::parse_u8_slice(b, &Default::default()).unwrap());
        }
    }
    #[test]
    fn buffered_fields_retrace_at_the_end_and_keep_both_channels() {
        let raw = fields::legacy_fields().unwrap();
        let f = Fields::from_slice(&raw).unwrap();
        assert_eq!((f.size, f.count, f.rate), ([512, 288], 96, 24));
        assert_eq!(f.frame(7).len(), 512 * 288 * 2);
        assert!(f.frame(7).chunks_exact(2).any(|p| p[0] > 185 && p[1] < 72));
        assert!(f.frame(7).chunks_exact(2).any(|p| p[1] > 185 && p[0] < 72));
        assert_eq!(f.sample(0.0), (0, 1, 0.0));
        assert_eq!(f.sample(95.0 / 24.), (95, 95, 0.0));
        assert_eq!(f.sample(190.0 / 24.), (0, 1, 0.0));
        assert_eq!(f.sample(45.5 / 24.), (45, 46, 0.5));
        let position = |source_frame: f64| {
            let (a, _, fraction) = f.sample(source_frame / 24.);
            f64::from(a) + f64::from(fraction)
        };
        for step in 1..=1900 {
            let x = f64::from(step) / 20.;
            assert!(position(x) >= position(x - 0.05));
            assert!((position(x) - position(190. - x)).abs() < 1e-5);
        }
        for endpoint in [0.0, 95.0] {
            assert!((position(endpoint + 0.1) - endpoint).abs() < 0.003);
        }
        assert!((position(8.001) - position(7.999) - 0.002).abs() < 1e-5);
    }
}
