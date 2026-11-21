use super::*;

/// One physical wheel in projector order, starting at the lamp.
#[derive(Clone, Debug, PartialEq)]
pub struct WheelLayer {
    pub id: u64,
    pub wheel: String,
    pub swatches: Option<[u32; 4]>,
    pub motion: MotionPreset,
    /// The two authored oil solves, retained across coral preset switches.
    pub material_motion: MaterialMotion,
    pub films: [bool; 2],
    pub opacity: f32,
    pub envelope: Option<OpacityEnvelope>,
}
impl WheelLayer {
    pub(super) fn opacity_at(&self, time: f64) -> f32 {
        self.opacity * self.envelope.as_ref().map_or(1., |e| e.level_at(time))
    }
}

pub(super) struct Exposure<'a> {
    pub gpu: &'a StackGpu,
    pub incident: &'a gpu::TextureView,
    pub output: &'a gpu::TextureView,
}

pub(super) struct StackGpu {
    passes: [Pass; 4],
    pub(super) encode: Pass,
    pub(super) images: [Target; 2],
}
impl StackGpu {
    pub(super) fn images(device: &gpu::Device, size: [u32; 2]) -> [Target; 2] {
        std::array::from_fn(|_| Target::new(device, "Transmitted wheel light", size, FIELD_FORMAT))
    }

    pub(super) fn new(device: &gpu::Device, size: [u32; 2]) -> Self {
        let quad = shader(device, "Wheel stack quad", include_str!("vertex.wgsl"));
        let output = shader(
            device,
            "Encode linear light",
            include_str!("feedback_output.wgsl"),
        );
        let image = Binding::Image(gpu::TextureViewDimension::D2);
        let basic = [image, Binding::Sampler, Binding::Buffer, image];
        let finite = [
            image,
            Binding::Sampler,
            Binding::Buffer,
            Binding::Image(gpu::TextureViewDimension::D2Array),
            image,
            Binding::Buffer,
            image,
            image,
        ];
        let build = |code: &[u8], bindings: &[Binding]| {
            let fragment = device.create_shader_module(gpu::ShaderModuleDescriptor {
                label: Some("Optical wheel transmission"),
                source: gpu::util::make_spirv(code),
            });
            pipeline(
                device,
                &quad,
                &fragment,
                "main",
                "Optical wheel transmission",
                FIELD_FORMAT,
                bindings,
            )
        };
        Self {
            passes: [
                build(
                    include_bytes!(concat!(env!("OUT_DIR"), "/violet-blue-stack.spv")),
                    &basic,
                ),
                build(
                    include_bytes!(concat!(env!("OUT_DIR"), "/wheel-stack.spv")),
                    &basic,
                ),
                build(
                    include_bytes!(concat!(env!("OUT_DIR"), "/violet-blue-footprint-stack.spv")),
                    &finite,
                ),
                build(
                    include_bytes!(concat!(env!("OUT_DIR"), "/wheel-footprint-stack.spv")),
                    &finite,
                ),
            ],
            encode: pipeline(
                device,
                &quad,
                &output,
                "encode_program",
                "Project wheel stack",
                COLOR_FORMAT,
                &[image, Binding::Sampler],
            ),
            images: Self::images(device, size),
        }
    }
    pub fn optics(&self, finite: bool, baseline: bool) -> &Pass {
        &self.passes[2 * usize::from(finite) + usize::from(!baseline)]
    }
}
impl LiveWheel {
    pub fn set_stack(&mut self, layers: Vec<WheelLayer>) -> Result<(), String> {
        if self.stack == layers {
            return Ok(());
        }
        let mut ids = std::collections::HashSet::new();
        if layers
            .iter()
            .any(|l| !ids.insert(l.id) || !(0.0..=1.0).contains(&l.opacity))
        {
            return Err("Invalid wheel stack ID, wheel or opacity".into());
        }
        let materials = layers
            .iter()
            .map(|l| {
                Ok((
                    l.id,
                    material::resolve(&self.materials, &l.wheel, l.swatches)?,
                ))
            })
            .collect::<Result<HashMap<_, _>, String>>()?;
        let saved = (self.motion, self.material_motion);
        let result = layers
            .iter()
            .try_for_each(|layer| self.switch_film_sources(layer.motion, layer.material_motion));
        self.switch_film_sources(saved.0, saved.1)?;
        result?;
        self.stack = layers;
        self.stack_materials = materials;
        self.stack_times.retain(|id, _| ids.contains(id));
        self.reset_feedback_history();
        Ok(())
    }

    /// Both source times are sampled from the same presentation instant.
    pub fn set_layer_times(&mut self, id: u64, times: [f64; 2]) -> Result<(), String> {
        if times.iter().any(|t| !t.is_finite()) {
            return Err("Invalid wheel layer time".into());
        }
        self.stack_times.insert(id, times);
        Ok(())
    }

    pub(super) fn render_stack(
        &mut self,
        out: Option<&gpu::TextureView>,
        selected_times: [f64; 2],
        program_time: f64,
    ) {
        use gpu::BindingResource::{Sampler, TextureView};
        let size = self.preview_size();
        let mut gpu = self
            .stack_gpu
            .take()
            .unwrap_or_else(|| StackGpu::new(&self.device, size));
        if gpu.images[0].texture.width() != size[0] || gpu.images[0].texture.height() != size[1] {
            gpu.images = StackGpu::images(&self.device, size);
        }
        let layers = std::mem::take(&mut self.stack);
        let selected = self.active;
        let selected_wheel = std::mem::take(&mut self.wheel);
        let selected_optics = (self.swatches, self.active_material);
        let selected_motion = (self.motion, self.material_motion);
        let selected_films = self.slide_visibility;
        // A transparent wheel neither attenuates light nor takes a focal plane.
        // With every wheel transparent, render the lamp through one zero-strength pass.
        let visible_count = layers.iter().filter(|l| l.opacity > 0.).count();
        let count = visible_count.max(1);
        let single = visible_count == 1 && layers.iter().any(|l| l.opacity_at(program_time) == 1.);
        let intervals = (count * 2 - 1) as f32;
        for (index, layer) in layers
            .iter()
            .filter(|l| visible_count == 0 || l.opacity > 0.)
            .take(count)
            .enumerate()
        {
            self.active = layer.id;
            self.wheel.clone_from(&layer.wheel);
            self.swatches = layer.swatches;
            self.active_material = self.stack_materials[&layer.id];
            self.switch_film_sources(layer.motion, layer.material_motion)
                .expect("stack geometry validated at selection");
            self.slide_visibility = layer.films;
            let mut material = self.active_material;
            material.lens = [
                self.focus.unwrap_or(-1.),
                layer.opacity_at(program_time),
                (2 * index) as f32 / intervals,
                (2 * index + 1) as f32 / intervals,
            ];
            self.queue
                .write_buffer(&self.material_uniform, 0, bytemuck::bytes_of(&material));
            let times = if layer.id == selected {
                selected_times
            } else {
                self.stack_times
                    .get(&layer.id)
                    .copied()
                    .unwrap_or(selected_times)
            };
            if single {
                self.upload_material();
                self.render_one(out, times[0], times[1], program_time, None);
                break;
            }
            self.render_one(
                None,
                times[0],
                times[1],
                program_time,
                Some(OpticalExposure::Stack(Exposure {
                    gpu: &gpu,
                    incident: &gpu.images[1 - index % 2].view,
                    output: &gpu.images[index % 2].view,
                })),
            );
        }
        self.active = selected;
        self.wheel = selected_wheel;
        (self.swatches, self.active_material) = selected_optics;
        self.switch_film_sources(selected_motion.0, selected_motion.1)
            .expect("restore active wheel geometry");
        self.slide_visibility = selected_films;
        self.upload_material();
        self.stack = layers;
        if single {
            self.stack_gpu = Some(gpu);
            return;
        }
        let light = &gpu.images[(count - 1) % 2].view;
        let mut enc = self.device.create_command_encoder(&Default::default());
        if let Some(feedback) = &mut self.feedback {
            feedback.compose_from(
                &self.device,
                &self.queue,
                &mut enc,
                &self.sampler,
                &self.optics.view,
                program_time,
                Some(light),
            );
        } else {
            let group = bind(
                &self.device,
                &gpu.encode,
                &[TextureView(light), Sampler(&self.sampler)],
            );
            draw(
                &mut enc,
                "Encode stacked transmission",
                &self.optics.view,
                &gpu.encode,
                &group,
            );
        }
        if let Some(out) = out {
            let group = bind(
                &self.device,
                &self.blit_pass,
                &[TextureView(&self.optics.view), Sampler(&self.sampler)],
            );
            draw(
                &mut enc,
                "Present wheel stack",
                out,
                &self.blit_pass,
                &group,
            );
        }
        self.queue.submit(Some(enc.finish()));
        self.stack_gpu = Some(gpu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exposure(r: &mut LiveWheel) -> Vec<u8> {
        r.render_film_times_offscreen(1.15, 0.64, 0.).unwrap();
        r.capture_rgba8().unwrap().2
    }

    #[test]
    #[ignore = "uses an isolated SwiftShader CPU Vulkan ICD"]
    fn timeline_gain_uses_song_time_and_matches_explicit_optical_opacity() {
        let size = [128, 72];
        let mut r = crate::feedback_test::fixture(size);
        let layer = WheelLayer {
            id: 1,
            wheel: "violet-blue".into(),
            swatches: None,
            motion: MotionPreset::WheelFlow,
            material_motion: MaterialMotion::default(),
            films: [true; 2],
            opacity: 1.,
            envelope: None,
        };
        let gain: OpacityEnvelope = serde_json::from_value(serde_json::json!([{
            "start":1.,"end":4.,"fadeIn":2.,"fadeOut":1.,"level":1.
        }]))
        .unwrap();
        r.select_track(1);
        let frame = |r: &mut LiveWheel, program_time: f64| {
            r.render_film_times_offscreen(0.6, 0.6, program_time)
                .unwrap();
            r.capture_rgba8().unwrap().2
        };
        r.set_stack(vec![WheelLayer {
            opacity: 0.5,
            ..layer.clone()
        }])
        .unwrap();
        let half = frame(&mut r, 20.);
        r.set_stack(vec![WheelLayer {
            envelope: Some(gain),
            ..layer.clone()
        }])
        .unwrap();
        let silent = frame(&mut r, 0.);
        assert_eq!(frame(&mut r, 2.), half);
        assert_ne!(silent, half);
        assert_eq!(frame(&mut r, 4.), silent);
        let full = frame(&mut r, 3.);
        r.set_stack(vec![layer]).unwrap();
        assert_eq!(frame(&mut r, 3.), full);
    }

    #[test]
    #[ignore = "uses an isolated SwiftShader CPU Vulkan ICD"]
    fn independent_wheel_stack_preserves_single_output_and_movie_state() {
        let size = [128, 72];
        let mut r = crate::feedback_test::fixture(size);
        let base = WheelLayer {
            id: 1,
            wheel: "violet-blue".into(),
            swatches: None,
            motion: MotionPreset::WheelFlow,
            material_motion: MaterialMotion::default(),
            films: [true, true],
            opacity: 1.,
            envelope: None,
        };
        let overlay = WheelLayer {
            id: 2,
            wheel: "yellow-red".into(),
            swatches: None,
            motion: MotionPreset::WheelWithCoralBeads,
            material_motion: MaterialMotion::default(),
            films: [true, true],
            opacity: 0.68,
            envelope: None,
        };
        r.select_track(base.id);
        r.set_footprint_sampling(true);
        r.set_focus(Some(0.71)).unwrap();
        let solo = exposure(&mut r);
        r.set_stack(vec![base.clone()]).unwrap();
        assert_eq!(exposure(&mut r), solo);
        r.set_stack(vec![base.clone(), overlay.clone()]).unwrap();
        r.set_layer_times(overlay.id, [0.78, 0.13]).unwrap();
        let both = exposure(&mut r);
        assert_ne!(both, solo);
        r.set_layer_times(overlay.id, [1.4, 2.3]).unwrap();
        assert_ne!(exposure(&mut r), both);
        r.set_layer_times(overlay.id, [0.78, 0.13]).unwrap();
        r.set_stack(vec![overlay.clone(), base.clone()]).unwrap();
        assert_ne!(exposure(&mut r), both);
        r.set_stack(vec![base.clone(), overlay.clone()]).unwrap();
        assert_eq!(exposure(&mut r), both);
        {
            let mut movie = pollster::block_on(r.begin_movie(size, true)).unwrap();
            movie
                .frame_films(1.15, 0.64, 0., |pixels| {
                    assert_eq!(pixels, both);
                    Ok(())
                })
                .unwrap();
            movie.select_gesture_layer(2).unwrap();
            movie.set_slide_pressure(1, 0.9, [0.54, 0.6]).unwrap();
            movie
                .frame_films(1.15, 0.64, 0., |pixels| {
                    assert_ne!(pixels, both);
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(exposure(&mut r), both);
        r.set_stack(vec![
            base.clone(),
            WheelLayer {
                opacity: 0.,
                ..overlay.clone()
            },
        ])
        .unwrap();
        let transparent = exposure(&mut r);
        assert_ne!(transparent, both);
        assert_eq!(transparent, solo);
        r.set_layer_times(2, [2.5, 2.8]).unwrap();
        assert_eq!(exposure(&mut r), transparent);
        r.select_track(2);
        r.set_slide_pressure(1, 0.8, [0.5, 0.4]).unwrap();
        r.select_track(1);
        r.set_stack(vec![base]).unwrap();
        assert_eq!(exposure(&mut r), solo);
    }
}
