//! Half side-by-side presentation of the editor's live film stack.
use super::*;

// Signed half eye separation times focal length in gate-width-normalized units × mm.
// Matches the stereo pressure study viewed on the Samsung.
pub(super) const EYE_SCALE: f32 = 14.;

pub struct StereoOutput {
    depth: f32,
    cfg: gpu::SurfaceConfiguration,
    pack: Pass,
    left: Target,
    right_feedback: Option<feedback::Feedback>,
    feedback_generation: Option<u64>,
    has_frame: bool,
}

impl StereoOutput {
    pub fn new(r: &LiveWheel, surface: &gpu::Surface<'_>, size: [u32; 2]) -> Result<Self, String> {
        let adapter = r.surface_adapter.as_ref().ok_or("Editor GPU unavailable")?;
        if !adapter.is_surface_supported(surface) {
            return Err("The editor GPU cannot present on this stereo display".into());
        }
        check_extent(size, r.device.limits().max_texture_dimension_2d)?;
        let mut cfg = surface
            .get_default_config(adapter, size[0].max(1), size[1].max(1))
            .ok_or("Unsupported stereo display surface")?;
        cfg.present_mode = gpu::PresentMode::Fifo;
        cfg.desired_maximum_frame_latency = 1;
        surface.configure(&r.device, &cfg);
        Ok(Self::with_config(r, cfg))
    }

    fn with_config(r: &LiveWheel, cfg: gpu::SurfaceConfiguration) -> Self {
        let pack = pipeline(
            &r.device,
            &shader(&r.device, "Stereo quad", include_str!("vertex.wgsl")),
            &shader(&r.device, "Stereo display", include_str!("blit.wgsl")),
            if cfg.format.is_srgb() {
                "decode_for_srgb"
            } else {
                "copy_encoded"
            },
            "Pack half side-by-side stereo",
            cfg.format,
            &[
                Binding::Image(gpu::TextureViewDimension::D2),
                Binding::Sampler,
            ],
        );
        Self {
            depth: 0.2,
            cfg,
            pack,
            left: Target::new(&r.device, "Left eye", r.preview_size(), COLOR_FORMAT),
            right_feedback: None,
            feedback_generation: None,
            has_frame: false,
        }
    }

    pub fn set_depth(&mut self, depth: f32) -> Result<(), String> {
        if !(0.0..=1.0).contains(&depth) { return Err("Expected stereo depth in [0, 1]".into()); }
        self.depth = depth;
        Ok(())
    }

    /// Render both eyes from one latched presentation and present one half-SBS frame.
    pub fn present(
        &mut self,
        r: &mut LiveWheel,
        surface: &gpu::Surface<'_>,
        size: [u32; 2],
        rear: f64,
        front: f64,
        program: f64,
    ) -> Result<bool, String> {
        self.advance(r, rear, front, program)?;
        if size.contains(&0) {
            return Ok(false);
        }
        if [self.cfg.width, self.cfg.height] != size {
            check_extent(size, r.device.limits().max_texture_dimension_2d)?;
            [self.cfg.width, self.cfg.height] = size;
            surface.configure(&r.device, &self.cfg);
        }
        let Some(frame) = acquire(r, surface, &self.cfg)? else {
            return Ok(false);
        };
        self.pack_eyes(r, &frame.texture.create_view(&Default::default()), size);
        frame.present();
        Ok(true)
    }

    /// Advance both eyes on their shared clock when the TV has no visible drawable.
    pub fn advance(&mut self, r: &mut LiveWheel, rear: f64, front: f64, program: f64) -> Result<(), String> {
        r.verify_live_sources()?;
        if ![rear, front, program].iter().all(|t| t.is_finite()) {
            return Err("Invalid stereo frame time".into());
        }
        self.render_pair(r, rear, front, program);
        Ok(())
    }

    fn render_pair(&mut self, r: &mut LiveWheel, rear: f64, front: f64, program: f64) {
        let size = r.preview_size();
        if [self.left.texture.width(), self.left.texture.height()] != size {
            self.left = Target::new(&r.device, "Left eye", size, COLOR_FORMAT);
        }
        // A recursive exposure feeds back into the same eye on subsequent frames.
        if let Some(left) = &mut r.feedback {
            if self.feedback_generation != Some(r.feedback_generation) {
                left.reset();
                let right = self
                    .right_feedback
                    .get_or_insert_with(|| feedback::Feedback::new(&r.device, size, left.settings));
                right.reset();
                self.feedback_generation = Some(r.feedback_generation);
            }
            if let Some(right) = &mut self.right_feedback {
                right.settings = left.settings;
            }
        } else {
            self.right_feedback = None;
            self.feedback_generation = None;
        }
        r.stereo_eye_scale = EYE_SCALE * self.depth;
        r.upload_material();
        r.render_films_to(None, rear, front, program);
        std::mem::swap(&mut self.left, &mut r.optics);
        std::mem::swap(&mut self.right_feedback, &mut r.feedback);
        r.stereo_eye_scale = -EYE_SCALE * self.depth;
        r.upload_material();
        r.render_latched_films_to(None, rear, front, program);
        std::mem::swap(&mut self.right_feedback, &mut r.feedback);
        r.stereo_eye_scale = 0.;
        r.upload_material();
        self.has_frame = true;
    }

    fn pack_eyes(&self, r: &LiveWheel, dst: &gpu::TextureView, size: [u32; 2]) {
        let eyes = [&self.left.view, &r.optics.view].map(|view| {
            bind(
                &r.device,
                &self.pack,
                &[
                    gpu::BindingResource::TextureView(view),
                    gpu::BindingResource::Sampler(&r.sampler),
                ],
            )
        });
        let mut encoder = r.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&gpu::RenderPassDescriptor {
                label: Some("Present simultaneous stereo eyes"),
                color_attachments: &[Some(gpu::RenderPassColorAttachment {
                    view: dst,
                    resolve_target: None,
                    ops: gpu::Operations {
                        load: gpu::LoadOp::Clear(gpu::Color::BLACK),
                        store: gpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pack.pipeline);
            let [w, h] = size.map(|x| x as f32);
            // Each half expands to a full display in the television. Centre any
            // pillarboxing within each individual half so both optics axes coincide.
            let width = w.min(h * 16. / 9.);
            let height = (width * 9. / 16.).min(h);
            for (i, eye) in eyes.iter().enumerate() {
                let x = i as f32 * w * 0.5 + (w - width) * 0.25;
                pass.set_viewport(x, (h - height) * 0.5, width * 0.5, height, 0., 1.);
                pass.set_bind_group(0, eye, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        r.queue.submit(Some(encoder.finish()));
    }

    /// Show the most recent left-eye frame on the editor without advancing the solver.
    pub fn preview(&self, r: &LiveWheel, surface: &gpu::Surface<'_>) -> Result<bool, String> {
        if !self.has_frame || r.actual_size.contains(&0) {
            return Ok(false);
        }
        let Some(frame) = acquire(r, surface, &r.cfg)? else {
            return Ok(false);
        };
        let group = bind(
            &r.device,
            &r.blit_pass,
            &[
                gpu::BindingResource::TextureView(&self.left.view),
                gpu::BindingResource::Sampler(&r.sampler),
            ],
        );
        let mut encoder = r.device.create_command_encoder(&Default::default());
        draw(
            &mut encoder,
            "Editor left-eye preview",
            &frame.texture.create_view(&Default::default()),
            &r.blit_pass,
            &group,
        );
        r.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(true)
    }
}

fn acquire(
    r: &LiveWheel,
    surface: &gpu::Surface<'_>,
    cfg: &gpu::SurfaceConfiguration,
) -> Result<Option<gpu::SurfaceTexture>, String> {
    match surface.get_current_texture() {
        Ok(frame) => Ok(Some(frame)),
        Err(gpu::SurfaceError::Timeout) => Ok(None),
        Err(gpu::SurfaceError::Outdated | gpu::SurfaceError::Lost) => {
            surface.configure(&r.device, cfg);
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(all(test, feature = "live-solver"))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an isolated software Vulkan ICD"]
    fn packed_stereo_latches_one_presentation_and_preserves_eye_order() {
        let size = [256, 144];
        let mut r = crate::feedback_test::fixture(size);
        r.set_wheel_palette("orchid", Some([0xffffff, 0xff728c, 0xff728c, 0xffffff]))
            .unwrap();
        r.set_slide_visibility(true, false);
        r.set_focus(Some(0.72)).unwrap();
        let mut cfg = r.cfg.clone();
        cfg.format = COLOR_FORMAT;
        let mut stereo = StereoOutput::with_config(&r, cfg);
        for depth in [0.5, 1.] {
            r.set_film_depth(0, Some(depth)).unwrap();
            let presentation = r.presentation;
            stereo.render_pair(&mut r, 0.6, 0.2, 0.);
            assert_eq!(r.presentation, presentation + 1);
            assert_eq!(r.stereo_eye_scale, 0.);
            let right = r.capture_rgba8().unwrap().2;
            std::mem::swap(&mut r.optics, &mut stereo.left);
            let left = r.capture_rgba8().unwrap().2;
            std::mem::swap(&mut r.optics, &mut stereo.left);
            assert_eq!(
                left == right,
                depth == 0.5,
                "convergence at middle film only"
            );

            let packed = Target::new(&r.device, "Packed stereo readback", size, COLOR_FORMAT);
            stereo.pack_eyes(&r, &packed.view, size);
            let saved = std::mem::replace(&mut r.optics, packed);
            let result = r.capture_rgba8().unwrap().2;
            r.optics = saved;
            let width = size[0] as usize;
            for (eye, pixels) in [left, right].iter().enumerate() {
                for y in 0..size[1] as usize {
                    for x in 0..width / 2 {
                        let input = 4 * (y * width + 2 * x);
                        let output = 4 * (y * width + eye * width / 2 + x);
                        for c in 0..4 {
                            let avg = (u16::from(pixels[input + c])
                                + u16::from(pixels[input + 4 + c]))
                                / 2;
                            assert!(u16::from(result[output + c]).abs_diff(avg) <= 1);
                        }
                    }
                }
            }
        }
    }
}
