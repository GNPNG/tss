use super::*;
use std::collections::VecDeque;

// 28.125 MiB of history at 640 x 360 plus two full-size linear work images.
const CAPACITY: usize = 16;
const MAX_HISTORY_WIDTH: u32 = 640;
pub const MIN_DELAY_MS: u32 = 25;
pub const MAX_DELAY_MS: u32 = 450;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeedbackSettings {
    pub delay_ms: u32,
    pub strength: f32,
    pub zoom: f32,
    pub rotation_degrees: f32,
}

impl FeedbackSettings {
    pub fn validate(self) -> Result<(), String> {
        if !(MIN_DELAY_MS..=MAX_DELAY_MS).contains(&self.delay_ms)
            || !(0.0..=0.95).contains(&self.strength)
            || !(0.85..=1.15).contains(&self.zoom)
            || !(-12.0..=12.0).contains(&self.rotation_degrees)
        {
            return Err(
                "Video feedback settings exceed the supported delay, strength or transform range"
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Default)]
struct History {
    frames: VecDeque<(f64, usize)>,
    next_slot: usize,
    next_sample: Option<f64>,
    previous: Option<f64>,
}
impl History {
    // The newest retained program image at or before the requested PTS.
    fn tap(&mut self, time: f64, delay: f64) -> Option<usize> {
        if self
            .previous
            .is_some_and(|p| time < p || time - p > MAX_DELAY_MS as f64 / 1000. + 0.15)
        {
            *self = Self::default();
        }
        self.previous = Some(time);
        self.frames
            .iter()
            .rev()
            .find(|(pts, _)| *pts <= time - delay + 1e-9)
            .map(|(_, slot)| *slot)
    }
    fn retain(&mut self, pts: f64, delay: f64) -> Option<usize> {
        if self.next_sample.is_some_and(|next| pts + 1e-9 < next) {
            return None;
        }
        // Leave two slots for the floor tap and one in-flight frame. Select by
        // timestamps rather than treating a count of presents as elapsed time.
        let interval = (delay / (CAPACITY - 3) as f64).max(1. / 120.);
        self.next_sample = Some(pts + interval);
        let slot = self.next_slot;
        self.next_slot = (slot + 1) % CAPACITY;
        if self.frames.len() == CAPACITY {
            self.frames.pop_front();
        }
        self.frames.push_back((pts, slot));
        Some(slot)
    }
}

struct Images {
    input: Target,
    program: Target,
    history: Vec<Target>,
}
impl Images {
    fn new(device: &gpu::Device, size: [u32; 2]) -> Self {
        let width = size[0].min(MAX_HISTORY_WIDTH);
        let history_size = [width, (width * size[1] / size[0]).max(1)];
        Self {
            input: Target::new(
                device,
                "Linear projected light before feedback",
                size,
                FIELD_FORMAT,
            ),
            program: Target::new(device, "Linear recursive program", size, FIELD_FORMAT),
            history: (0..CAPACITY)
                .map(|_| {
                    Target::new(
                        device,
                        "Timestamped program image",
                        history_size,
                        FIELD_FORMAT,
                    )
                })
                .collect(),
        }
    }
}
pub(crate) struct Feedback {
    pub settings: FeedbackSettings,
    images: Images,
    timeline: History,
    mix: Pass,
    store: Pass,
    encode: Pass,
    linear_optics: [Pass; 4],
    params: gpu::Buffer,
}
pub(super) fn linear_optics(
    device: &gpu::Device,
    vertex: &gpu::ShaderModule,
    format: gpu::TextureFormat,
) -> [Pass; 4] {
    let image = Binding::Image(gpu::TextureViewDimension::D2);
    let base = [image, Binding::Sampler, Binding::Buffer];
    let footprint = [
        image,
        Binding::Sampler,
        Binding::Buffer,
        Binding::Image(gpu::TextureViewDimension::D2Array),
        image,
        Binding::Buffer,
        image,
    ];
    let optical = |label: &str, code: &[u8], inputs: &[Binding]| {
        let module = device.create_shader_module(gpu::ShaderModuleDescriptor {
            label: Some(label),
            source: gpu::util::make_spirv(code),
        });
        pipeline(device, vertex, &module, "main", label, format, inputs)
    };
    [
        optical(
            "Linear Violet / Blue",
            include_bytes!(concat!(env!("OUT_DIR"), "/violet-blue.spv")),
            &base,
        ),
        optical(
            "Linear wheel",
            include_bytes!(concat!(env!("OUT_DIR"), "/wheel.spv")),
            &base,
        ),
        optical(
            "Linear finite-pixel Violet / Blue",
            include_bytes!(concat!(env!("OUT_DIR"), "/violet-blue-footprint.spv")),
            &footprint,
        ),
        optical(
            "Linear finite-pixel wheel",
            include_bytes!(concat!(env!("OUT_DIR"), "/wheel-footprint.spv")),
            &footprint,
        ),
    ]
}
impl Feedback {
    pub fn new(device: &gpu::Device, size: [u32; 2], settings: FeedbackSettings) -> Self {
        let vertex = shader(device, "Program quad", include_str!("vertex.wgsl"));
        let effect = shader(
            device,
            "Timestamped video feedback",
            include_str!("feedback.wgsl"),
        );
        let output = shader(
            device,
            "Linear-program output",
            include_str!("feedback_output.wgsl"),
        );
        let image = Binding::Image(gpu::TextureViewDimension::D2);
        let linear_optics = linear_optics(device, &vertex, FIELD_FORMAT);
        Self {
            settings,
            images: Images::new(device, size),
            timeline: History::default(),
            linear_optics,
            mix: pipeline(
                device,
                &vertex,
                &effect,
                "feedback",
                "Recursive feedback",
                FIELD_FORMAT,
                &[image, image, Binding::Sampler, Binding::Buffer],
            ),
            store: pipeline(
                device,
                &vertex,
                &output,
                "keep_linear",
                "Store linear program history",
                FIELD_FORMAT,
                &[image, Binding::Sampler],
            ),
            encode: pipeline(
                device,
                &vertex,
                &output,
                "encode_program",
                "Encode program for preview",
                COLOR_FORMAT,
                &[image, Binding::Sampler],
            ),
            params: uniform(device, "Video loop controls", &[0.0f32; 4]),
        }
    }
    pub fn prepare(&mut self, device: &gpu::Device, size: [u32; 2]) {
        if self.images.input.texture.width() != size[0]
            || self.images.input.texture.height() != size[1]
        {
            self.images = Images::new(device, size);
            self.reset();
        }
    }
    pub fn reset(&mut self) {
        self.timeline = History::default();
    }
    pub fn optics(&self, finite_pixel: bool, baseline: bool) -> (&Pass, &gpu::TextureView) {
        (
            &self.linear_optics[2 * usize::from(finite_pixel) + usize::from(!baseline)],
            &self.images.input.view,
        )
    }
    pub fn compose_from(
        &mut self,
        device: &gpu::Device,
        queue: &gpu::Queue,
        enc: &mut gpu::CommandEncoder,
        smp: &gpu::Sampler,
        present: &gpu::TextureView,
        pts: f64,
        input: Option<&gpu::TextureView>,
    ) {
        use gpu::BindingResource::{Sampler, TextureView};
        let delay = f64::from(self.settings.delay_ms) / 1000.;
        let prior = self.timeline.tap(pts, delay);
        let (s, c) = (-self.settings.rotation_degrees.to_radians()).sin_cos();
        let gain = if prior.is_some() {
            self.settings.strength
        } else {
            0.
        };
        let params = [gain, 1. / self.settings.zoom, c, s];
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        let past = &self.images.history[prior.unwrap_or(0)].view;
        let mix = bind(
            device,
            &self.mix,
            &[
                TextureView(input.unwrap_or(&self.images.input.view)),
                TextureView(past),
                Sampler(smp),
                self.params.as_entire_binding(),
            ],
        );
        draw(
            enc,
            "Combine current light and prior program",
            &self.images.program.view,
            &self.mix,
            &mix,
        );
        let source = [TextureView(&self.images.program.view), Sampler(smp)];
        if let Some(slot) = self.timeline.retain(pts, delay) {
            let copy = bind(device, &self.store, &source);
            draw(
                enc,
                "Retain completed program",
                &self.images.history[slot].view,
                &self.store,
                &copy,
            );
        }
        let display = bind(device, &self.encode, &source);
        draw(enc, "Encode final program", present, &self.encode, &display);
    }
}
