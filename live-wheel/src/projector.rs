//! Delayed audience output on the existing device. Media time describes the
//! picture; monotonic deadlines continue to advance while an ended song drains.
use super::*;
use std::collections::VecDeque;

const MAX_DELAY_MS: u32 = 1000;
const MAX_SLOTS: usize = 43;
#[derive(Clone, Copy)]
struct Held {
    sampled_at: f64,
    song_time: f64,
    slot: usize,
}
struct DelayLine {
    delay: f64,
    capacity: usize,
    interval: f64,
    next_bucket: Option<i64>,
    next_slot: usize,
    frames: VecDeque<Held>,
}
impl DelayLine {
    fn new(ms: u32) -> Result<Self, String> {
        if ms > MAX_DELAY_MS {
            return Err("Projector delay must be 0–1000 ms".into());
        }
        let delay = f64::from(ms) / 1000.;
        let capacity = ((delay * 120.).ceil() as usize + 3).min(MAX_SLOTS);
        let interval = (delay / (MAX_SLOTS - 3) as f64).max(1. / 120.);
        Ok(Self {
            delay,
            capacity,
            interval,
            next_bucket: None,
            next_slot: 0,
            frames: Default::default(),
        })
    }
    fn reset(&mut self) {
        self.frames.clear();
        self.next_bucket = None;
        self.next_slot = 0;
    }
    fn retain(&mut self, now: f64, song: f64) -> Option<usize> {
        let bucket = ((now + 1e-9) / self.interval).floor() as i64;
        if self.next_bucket.is_some_and(|next| bucket < next)
            || self.frames.back().is_some_and(|f| f.song_time == song)
        {
            return None;
        }
        self.next_bucket = Some(bucket + 1);
        let slot = self.next_slot;
        self.next_slot = (slot + 1) % self.capacity;
        if self.frames.len() == self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(Held {
            sampled_at: now,
            song_time: song,
            slot,
        });
        Some(slot)
    }
    fn due(&self, now: f64) -> Option<Held> {
        self.frames
            .iter()
            .rev()
            .find(|f| f.sampled_at + self.delay <= now + 1e-9)
            .copied()
    }
}
#[derive(Default, Clone, Copy)]
pub struct ProjectorTiming {
    pub song_time: Option<f64>,
    pub age_ms: Option<f64>,
    pub retained: usize,
}
pub struct ProjectorOutput {
    cfg: gpu::SurfaceConfiguration,
    delay: DelayLine,
    images: Vec<Target>,
    keep: Pass,
    show: Pass,
}
impl ProjectorOutput {
    pub fn new(
        renderer: &LiveWheel,
        surface: &gpu::Surface<'_>,
        size: [u32; 2],
        delay_ms: u32,
    ) -> Result<Self, String> {
        let adapter = renderer
            .surface_adapter
            .as_ref()
            .ok_or("Editor surface adapter unavailable")?;
        if !adapter.is_surface_supported(surface) {
            return Err("The selected editor GPU cannot present on this projector".into());
        }
        let mut cfg = surface
            .get_default_config(adapter, size[0].max(1), size[1].max(1))
            .ok_or("Unsupported projector surface")?;
        let caps = surface.get_capabilities(adapter);
        if let Some(fmt) = caps.formats.iter().find(|&&fmt| {
            matches!(
                fmt,
                gpu::TextureFormat::Bgra8Unorm | gpu::TextureFormat::Rgba8Unorm
            )
        }) {
            cfg.format = *fmt;
        }
        cfg.present_mode = if caps.present_modes.contains(&gpu::PresentMode::Mailbox) {
            gpu::PresentMode::Mailbox
        } else {
            gpu::PresentMode::Fifo
        };
        cfg.desired_maximum_frame_latency = 1;
        let out = Self::with_config(renderer, cfg, delay_ms)?;
        surface.configure(&renderer.device, &out.cfg);
        Ok(out)
    }
    fn with_config(
        renderer: &LiveWheel,
        cfg: gpu::SurfaceConfiguration,
        ms: u32,
    ) -> Result<Self, String> {
        let d = &renderer.device;
        let vert = shader(d, "Projector quad", include_str!("vertex.wgsl"));
        let blit = shader(
            d,
            "Display-referred projector plate",
            include_str!("blit.wgsl"),
        );
        let bindings = [
            Binding::Image(gpu::TextureViewDimension::D2),
            Binding::Sampler,
        ];
        let keep = pipeline(
            d,
            &vert,
            &blit,
            "copy_encoded",
            "Store completed audience program",
            COLOR_FORMAT,
            &bindings,
        );
        let entry = if cfg.format.is_srgb() {
            "decode_for_srgb"
        } else {
            "copy_encoded"
        };
        let show = pipeline(
            d,
            &vert,
            &blit,
            entry,
            "Project audience program",
            cfg.format,
            &bindings,
        );
        let delay = DelayLine::new(ms)?;
        let images = Self::images(renderer, delay.capacity);
        Ok(Self {
            cfg,
            delay,
            images,
            keep,
            show,
        })
    }
    fn images(renderer: &LiveWheel, n: usize) -> Vec<Target> {
        let w = renderer.preview_size()[0].min(640);
        (0..n)
            .map(|_| {
                Target::new(
                    &renderer.device,
                    "Audience presentation history",
                    [w, (w * 9 / 16).max(1)],
                    COLOR_FORMAT,
                )
            })
            .collect()
    }
    pub fn set_delay(&mut self, renderer: &LiveWheel, ms: u32) -> Result<(), String> {
        if (self.delay.delay * 1000.).round() as u32 == ms {
            return Ok(());
        }
        let next = DelayLine::new(ms)?;
        if self.images.len() != next.capacity {
            // Free the old queue before allocating a differently sized queue.
            self.images.clear();
            self.images = Self::images(renderer, next.capacity);
        }
        self.delay = next;
        Ok(())
    }
    pub fn reset(&mut self) {
        self.delay.reset();
    }
    pub fn present(
        &mut self,
        renderer: &LiveWheel,
        surface: &gpu::Surface<'_>,
        size: [u32; 2],
        now: f64,
        song: f64,
        new_frame: bool,
    ) -> Result<Option<ProjectorTiming>, String> {
        // Store before acquiring the audience surface. A late or occluded audience
        // swapchain cannot drop a correctly produced frame from the delay line.
        self.store(renderer, now, song, new_frame);
        if size.contains(&0) {
            return Ok(None);
        }
        if size != [self.cfg.width, self.cfg.height] {
            self.cfg.width = size[0];
            self.cfg.height = size[1];
            surface.configure(&renderer.device, &self.cfg);
        }
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(gpu::SurfaceError::Timeout) => return Ok(None),
            Err(gpu::SurfaceError::Lost | gpu::SurfaceError::Outdated) => {
                surface.configure(&renderer.device, &self.cfg);
                return Ok(None);
            }
            Err(e) => return Err(e.to_string()),
        };
        let view = frame.texture.create_view(&Default::default());
        let timing = self.show_at(renderer, &view, size, now);
        frame.present();
        Ok(Some(timing))
    }
    fn store(&mut self, r: &LiveWheel, now: f64, song: f64, new_frame: bool) {
        if !new_frame {
            return;
        }
        if let Some(slot) = self.delay.retain(now, song) {
            let mut enc = r.device.create_command_encoder(&Default::default());
            let inputs = [
                gpu::BindingResource::TextureView(&r.optics.view),
                gpu::BindingResource::Sampler(&r.sampler),
            ];
            let src = bind(&r.device, &self.keep, &inputs);
            draw(
                &mut enc,
                "Retain timestamped projector image",
                &self.images[slot].view,
                &self.keep,
                &src,
            );
            r.queue.submit(Some(enc.finish()));
        }
    }
    fn show_at(
        &self,
        r: &LiveWheel,
        dst: &gpu::TextureView,
        size: [u32; 2],
        now: f64,
    ) -> ProjectorTiming {
        let mut enc = r.device.create_command_encoder(&Default::default());
        let due = self.delay.due(now);
        let group = due.map(|f| {
            bind(
                &r.device,
                &self.show,
                &[
                    gpu::BindingResource::TextureView(&self.images[f.slot].view),
                    gpu::BindingResource::Sampler(&r.sampler),
                ],
            )
        });
        {
            let mut pass = enc.begin_render_pass(&gpu::RenderPassDescriptor {
                label: Some("Project timestamped image"),
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
            if let Some(group) = &group {
                let (w, h) = (size[0] as f32, size[1] as f32);
                let width = w.min(h * 16. / 9.);
                let height = (width * 9. / 16.).min(h);
                pass.set_viewport((w - width) * 0.5, (h - height) * 0.5, width, height, 0., 1.);
                pass.set_pipeline(&self.show.pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        r.queue.submit(Some(enc.finish()));
        ProjectorTiming {
            song_time: due.map(|x| x.song_time),
            age_ms: due.map(|x| (now - x.sampled_at) * 1000.),
            retained: self.delay.frames.len(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audience_deadlines_survive_transport_end() {
        let mut q = DelayLine::new(500).unwrap();
        for i in 0..165 {
            let t = i as f64 / 160.;
            q.retain(t, 2. + t);
        }
        assert!(q.frames.len() <= q.capacity);
        assert!(q.due(0.4).is_none());
        let due = q.due(1.).unwrap();
        assert!(due.sampled_at <= 0.5 && 0.5 - due.sampled_at < 0.025);
        assert_eq!(
            q.due(2.).unwrap().song_time,
            q.frames.back().unwrap().song_time
        );
        q.reset();
        assert!(q.due(3.).is_none());
    }
}

#[cfg(test)]
mod rendered_test {
    use super::*;
    #[test]
    #[ignore = "requires an isolated software Vulkan ICD"]
    fn projected_program_matches_delayed_images_and_drains() {
        let mut r = crate::feedback_test::fixture([128, 72]);
        let mut cfg = r.cfg.clone();
        cfg.format = COLOR_FORMAT;
        let mut p = ProjectorOutput::with_config(&r, cfg, 100).unwrap();
        r.set_feedback(Some(FeedbackSettings {
            delay_ms: 50,
            strength: 0.63,
            zoom: 1.0,
            rotation_degrees: 0.0,
        }))
        .unwrap();
        let mut base = Vec::new();
        let mut seen = Vec::new();
        let size = r.preview_size();
        for (i, (wheel, t)) in [
            ("violet-blue", 0.),
            ("blue-red", 0.06),
            ("blue-green", 0.112),
            ("yellow-red", 0.162),
        ]
        .into_iter()
        .enumerate()
        {
            r.set_wheel(wheel).unwrap();
            r.render_program_to(None, t, t);
            base.push(r.capture_rgba8().unwrap().2);
            p.store(&r, t, t, true);
            let due = p.show_at(&r, &r.optics.view, size, t);
            let output = r.capture_rgba8().unwrap().2;
            seen.push(due.song_time);
            if i < 2 {
                assert!(output.chunks_exact(4).all(|px| px == [0, 0, 0, 255]));
            } else {
                assert_eq!(
                    output,
                    base[i - 2],
                    "delayed frame differs from cue program"
                );
            }
        }
        let due = p.show_at(&r, &r.optics.view, size, 0.31);
        assert_eq!(due.song_time, Some(0.162));
        assert_eq!(r.capture_rgba8().unwrap().2, base[3]);
        assert_eq!(seen, [None, None, Some(0.), Some(0.06)]);
        p.reset();
        assert!(p
            .show_at(&r, &r.optics.view, size, 0.50)
            .song_time
            .is_none());
        println!("128x72 program with active recursive feedback: two black priming frames, three delayed frames equal the cue byte-for-byte including natural-end drain, reset clears the queue");
    }
}
