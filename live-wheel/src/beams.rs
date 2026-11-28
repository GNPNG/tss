//! Projectors illuminate a common screen independently. Framebuffers carry linear light.
use super::*;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamSettings {
    pub enabled: bool,
    pub lamp_kelvin: u32,
    pub focus: Option<f32>,
    /// Relative incident flux at a common reference screen area.
    pub intensity: f32,
    /// Horizontal and vertical displacement as fractions of the program frame.
    pub offset: [f32; 2],
}

#[derive(Default)]
pub(super) struct Routing {
    pub secondary: Option<BeamSettings>,
    pub secondary_ids: HashSet<u64>,
    pub secondary_gain: [f32; 4],
}

impl LiveWheel {
    pub fn set_primary_intensity(&mut self, intensity: f32) -> Result<(), String> {
        if !(0.0..=1.0).contains(&intensity) {
            return Err("Projector light output must be between zero and one".into());
        }
        self.primary_intensity = intensity;
        Ok(())
    }

    pub fn set_projector_routing(
        &mut self,
        secondary: Option<BeamSettings>,
        layer_ids: Vec<u64>,
    ) -> Result<(), String> {
        if secondary.is_none() && !layer_ids.is_empty() {
            return Err("Configure projector B before assigning layers to it".into());
        }
        let ids: HashSet<_> = layer_ids.into_iter().collect();
        if secondary == self.beams.secondary && self.beams.secondary_ids == ids {
            return Ok(());
        }
        let mut gain = [1.; 4];
        if let Some(beam) = secondary {
            if !(0.0..=1.0).contains(&beam.intensity)
                || beam.focus.is_some_and(|v| !(0.0..=1.0).contains(&v))
                || beam.offset.iter().any(|v| !(-1.0..=1.0).contains(v))
            {
                return Err("Invalid projector B focus, light output or registration".into());
            }
            gain = lamp::linear_gain(beam.lamp_kelvin)?;
        }
        self.beams = Routing {
            secondary,
            secondary_ids: ids,
            secondary_gain: gain,
        };
        Ok(())
    }
}

pub(super) struct ScreenSum {
    pass: Pass,
    copy: Pass,
    params: gpu::Buffer,
    pub first: Target,
}
impl ScreenSum {
    pub fn new(d: &gpu::Device, size: [u32; 2]) -> Self {
        let vertex = shader(d, "Projection screen", include_str!("vertex.wgsl"));
        let fragment = shader(d, "Add projected light", include_str!("beams.wgsl"));
        let linear = shader(
            d,
            "Hold projector exposure",
            include_str!("feedback_output.wgsl"),
        );
        let image = Binding::Image(gpu::TextureViewDimension::D2);
        Self {
            pass: pipeline(
                d,
                &vertex,
                &fragment,
                "add_beams",
                "Superimposed projectors",
                FIELD_FORMAT,
                &[image, image, Binding::Sampler, Binding::Buffer],
            ),
            copy: pipeline(
                d,
                &vertex,
                &linear,
                "keep_linear",
                "Hold first projector",
                FIELD_FORMAT,
                &[image, Binding::Sampler],
            ),
            params: uniform(
                d,
                "Projection registration and relative light output",
                &[0f32; 4],
            ),
            first: Target::new(d, "First projector exposure", size, FIELD_FORMAT),
        }
    }

    pub fn store_first(&self, r: &LiveWheel, source: &gpu::TextureView) {
        use gpu::BindingResource::{Sampler, TextureView};
        let mut enc = r.device.create_command_encoder(&Default::default());
        let group = bind(
            &r.device,
            &self.copy,
            &[TextureView(source), Sampler(&r.sampler)],
        );
        draw(
            &mut enc,
            "Store first exposure",
            &self.first.view,
            &self.copy,
            &group,
        );
        r.queue.submit(Some(enc.finish()));
    }

    pub fn compose(
        &self,
        r: &LiveWheel,
        b: &gpu::TextureView,
        output: &gpu::TextureView,
        intensity: f32,
        offset: [f32; 2],
    ) {
        use gpu::BindingResource::{Sampler, TextureView};
        let mut enc = r.device.create_command_encoder(&Default::default());
        r.queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&[offset[0], offset[1], r.primary_intensity, intensity]),
        );
        let group = bind(
            &r.device,
            &self.pass,
            &[
                TextureView(&self.first.view),
                TextureView(b),
                Sampler(&r.sampler),
                self.params.as_entire_binding(),
            ],
        );
        draw(
            &mut enc,
            "Add registered beams in linear light",
            output,
            &self.pass,
            &group,
        );
        r.queue.submit(Some(enc.finish()));
    }
}
