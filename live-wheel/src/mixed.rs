use super::*;
use crate::media_track::MediaSettings;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct MediaTrack {
    pub id: u64,
    pub opacity: f32,
    pub envelope: Option<OpacityEnvelope>,
    pub media: MediaSettings,
}
pub struct DecodedMediaFrame {
    pub id: u64,
    pub pts_us: i64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}
struct MediaImage {
    id: u64,
    pts_us: i64,
    size: [u32; 2],
    preview: bool,
    texture: gpu::Texture,
    view: gpu::TextureView,
}
#[derive(Default)]
pub(super) struct MediaScene {
    pub(super) tracks: Vec<MediaTrack>,
    order: Vec<u64>,
    images: HashMap<u64, MediaImage>,
    gpu: Option<MediaGpu>,
    screen: Option<beams::ScreenSum>,
    features: [f32; 4],
}
struct MediaGpu {
    compose: Pass,
    alpha: Pass,
    oil_mask: Pass,
    lamp: Pass,
    dark: Pass,
    params: gpu::Buffer,
    mattes: HashMap<u64, Target>,
    size: [u32; 2],
}
impl MediaGpu {
    fn new(d: &gpu::Device) -> Self {
        let quad = shader(d, "Artwork quad", include_str!("vertex.wgsl"));
        let media = shader(d, "Prepared visual media", include_str!("media.wgsl"));
        let light = shader(d, "Projector illumination", include_str!("lamp.wgsl"));
        let base = shader(
            d,
            "Program illumination and wheel mattes",
            include_str!("media-base.wgsl"),
        );
        let image = Binding::Image(gpu::TextureViewDimension::D2);
        let inputs = [
            image,
            image,
            image,
            image,
            Binding::Sampler,
            Binding::Buffer,
        ];
        let matte = gpu::TextureFormat::R8Unorm;
        Self {
            compose: pipeline(
                d,
                &quad,
                &media,
                "compose",
                "Composite artwork",
                FIELD_FORMAT,
                &inputs,
            ),
            alpha: pipeline(
                d,
                &quad,
                &media,
                "mask",
                "Prepared artwork alpha",
                matte,
                &inputs,
            ),
            oil_mask: pipeline(
                d,
                &quad,
                &base,
                "oil_mask",
                "Oil coverage matte",
                matte,
                &[image, Binding::Sampler, Binding::Buffer],
            ),
            lamp: pipeline(d, &quad, &light, "lamp", "Projector lamp", FIELD_FORMAT, &[Binding::Buffer]),
            dark: pipeline(d, &quad, &light, "dark", "Unlit screen", FIELD_FORMAT, &[]),
            params: uniform(d, "Artwork parameters", &[[0f32; 4]; 10]),
            mattes: HashMap::new(),
            size: [0, 0],
        }
    }
    fn begin_illumination(
        &self,
        device: &gpu::Device,
        queue: &gpu::Queue,
        encoder: &mut gpu::CommandEncoder,
        target: &gpu::TextureView,
        has_oil: bool,
        lamp_gain: [f32; 4],
    ) {
        let (pass, parameters) = if has_oil {
            queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&lamp_gain));
            (&self.lamp, bind(device, &self.lamp, &[self.params.as_entire_binding()]))
        } else {
            (&self.dark, bind(device, &self.dark, &[]))
        };
        draw(encoder, "Initialize program illumination", target, pass, &parameters);
    }
}
impl LiveWheel {
    /// Visible track IDs are in light-to-audience order. Hidden matte sources
    /// may still occur in tracks or in the wheel stack.
    pub fn set_media_scene(
        &mut self,
        tracks: Vec<MediaTrack>,
        order: Vec<u64>,
    ) -> Result<(), String> {
        if self.mixed.tracks == tracks && self.mixed.order == order {
            return Ok(());
        }
        let mut ids: HashSet<_> = self.stack.iter().map(|l| l.id).collect();
        for l in &tracks {
            if l.id == 0
                || !ids.insert(l.id)
                || !l.media.valid()
                || !(0.0..=1.).contains(&l.opacity)
            {
                return Err("Invalid visual media track".into());
            }
        }
        let mut visible = HashSet::new();
        if order
            .iter()
            .any(|id| !ids.contains(id) || !visible.insert(*id))
        {
            return Err("Invalid visual program order".into());
        }
        for l in &tracks {
            if [l.media.mask_a, l.media.mask_b]
                .into_iter()
                .flatten()
                .any(|id| id == l.id || !ids.contains(&id))
            {
                return Err("Invalid visual mask source".into());
            }
        }
        self.mixed.images.retain(|id, image| {
            tracks
                .iter()
                .any(|l| l.id == *id && l.media.asset_id == image.id)
        });
        self.mixed.tracks = tracks;
        self.mixed.order = order;
        self.reset_feedback_history();
        Ok(())
    }

    /// Timestamp on the decoder packet currently uploaded for this timeline track.
    pub fn media_sample_pts(&self, layer_id: u64) -> Option<i64> {
        self.mixed.images.get(&layer_id).map(|image| image.pts_us)
    }

    /// Audio values are bass, midrange, treble, and the accumulated bass amplitude in seconds.
    pub fn prepare_media(
        &mut self,
        time: f64,
        preview: bool,
        features: [f32; 4],
        mut decode: impl FnMut(u64, i64) -> Result<DecodedMediaFrame, String>,
    ) -> Result<(), String> {
        self.mixed.features = features;
        let mut preview_error = None;
        for layer in &self.mixed.tracks {
            let media = &layer.media;
            let prior = self.mixed.images.get(&layer.id);
            if media.still && prior.is_some_and(|p| p.id == media.asset_id && p.preview == preview)
            {
                continue;
            }
            let frame = match decode(
                media.asset_id,
                media.source_time(time, layer.envelope.as_ref()),
            ) {
                Ok(frame) => frame,
                Err(error) if preview => {
                    self.mixed.images.remove(&layer.id);
                    preview_error.get_or_insert(error);
                    continue;
                }
                Err(error) => return Err(error),
            };
            let size = [frame.width, frame.height];
            if frame.id != media.asset_id || size.contains(&0) {
                return Err("Invalid media frame identity or dimensions".into());
            }
            let expected = u64::from(frame.width) * u64::from(frame.height) * 4;
            if expected != frame.rgba.len() as u64 {
                return Err("Invalid decoded RGBA frame length".into());
            }
            check_extent(size, self.device.limits().max_texture_dimension_2d)?;
            if prior.is_some_and(|p| p.id == frame.id && p.pts_us == frame.pts_us && p.size == size)
            {
                self.mixed.images.get_mut(&layer.id).unwrap().preview = preview;
                continue;
            }
            if prior.is_none_or(|p| p.size != size) {
                let texture = self.device.create_texture(&gpu::TextureDescriptor {
                    label: Some("Timestamped straight-alpha sRGB media"),
                    size: extent(size, 1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: gpu::TextureDimension::D2,
                    format: gpu::TextureFormat::Rgba8UnormSrgb,
                    usage: gpu::TextureUsages::COPY_DST | gpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&Default::default());
                self.mixed.images.insert(
                    layer.id,
                    MediaImage {
                        id: frame.id,
                        pts_us: frame.pts_us,
                        size,
                        preview,
                        texture,
                        view,
                    },
                );
            }
            let image = self.mixed.images.get_mut(&layer.id).unwrap();
            self.queue.write_texture(
                image.texture.as_image_copy(),
                &frame.rgba,
                gpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.width * 4),
                    rows_per_image: Some(frame.height),
                },
                extent(size, 1),
            );
            image.id = frame.id;
            image.pts_us = frame.pts_us;
            image.preview = preview;
        }
        preview_error.map_or(Ok(()), Err)
    }
    fn draw_media(
        &self,
        g: &MediaGpu,
        layer: &MediaTrack,
        src: &MediaImage,
        under: &gpu::TextureView,
        out: &gpu::TextureView,
        mask: bool,
        time: f64,
        features: [f32; 4],
    ) {
        use gpu::BindingResource::{Sampler, TextureView};
        let m = &layer.media;
        let [w, h] = self.preview_size();
        let opacity = layer.opacity * layer.envelope.as_ref().map_or(1., |e| e.level_at(time));
        // A photographic transparency occupies the middle of its projector's gate.
        let blur = if self.carousel_lenses[0] {
            lens::GateLens::for_projector(true).radius(0.5,self.focus.unwrap_or(0.5))
        } else { 0. };
        let values = [
            [
                time as f32,
                opacity,
                w as f32 / h as f32 / (src.size[0] as f32 / src.size[1] as f32),
                2. / w.min(h) as f32,
            ],
            [
                m.key_color[0],
                m.key_color[1],
                m.key_color[2],
                m.key_threshold,
            ],
            [
                m.gradient_a[0],
                m.gradient_a[1],
                m.gradient_a[2],
                m.gradient_rate,
            ],
            [
                m.gradient_b[0],
                m.gradient_b[1],
                m.gradient_b[2],
                m.tunnel_bass,
            ],
            [m.bounce, m.blur, m.distortion, m.key_softness],
            [
                m.mask_a.is_some() as u8 as f32,
                m.mask_b.is_some() as u8 as f32,
                m.mask_op as u8 as f32,
                0.,
            ],
            features,
            [
                m.mode as u8 as f32,
                m.shape as u8 as f32,
                f32::from(m.key_enabled as u8),
                m.tunnel_speed,
            ],
            self.lamp_gain,
            [blur/1280.,blur/720.,0.,0.],
        ];
        self.queue
            .write_buffer(&g.params, 0, bytemuck::bytes_of(&values));
        let matte = |id: Option<u64>| {
            if mask {
                &self.zero.view
            } else {
                id.and_then(|x| g.mattes.get(&x))
                    .map_or(&self.zero.view, |v| &v.view)
            }
        };
        let pass = if mask { &g.alpha } else { &g.compose };
        let resources = [
            TextureView(&src.view),
            TextureView(under),
            TextureView(matte(m.mask_a)),
            TextureView(matte(m.mask_b)),
            Sampler(&self.sampler),
            g.params.as_entire_binding(),
        ];
        let group = bind(&self.device, pass, &resources);
        let mut enc = self.device.create_command_encoder(&Default::default());
        draw(
            &mut enc,
            "Key, color and composite artwork",
            out,
            pass,
            &group,
        );
        self.queue.submit(Some(enc.finish()));
    }
    fn render_beam(
        &mut self, scene: &MediaScene, g: &mut MediaGpu, s: &stack::StackGpu,
        wheels: &[WheelLayer], order: &[u64], selected: u64, selected_times: [f64; 2], time: f64,
    ) -> usize {
        let size = self.preview_size();
        let wheel_count = order.iter()
            .filter(|id| wheels.iter().any(|w| w.id == **id))
            .count();
        let mut enc = self.device.create_command_encoder(&Default::default());
        g.begin_illumination(
            &self.device,
            &self.queue,
            &mut enc,
            &s.images[0].view,
            wheel_count > 0,
            self.lamp_gain,
        );
        self.queue.submit(Some(enc.finish()));
        let refs: HashSet<_> = scene
            .tracks
            .iter()
            .filter(|l| order.contains(&l.id))
            .flat_map(|l| [l.media.mask_a, l.media.mask_b])
            .flatten()
            .collect();
        for id in refs {
            if !wheels.iter().any(|w| w.id == id) && !scene.images.contains_key(&id) {
                g.mattes.remove(&id);
                continue;
            }
            let mask = g.mattes.remove(&id).unwrap_or_else(|| {
                Target::new(
                    &self.device,
                    "Source alpha matte",
                    size,
                    gpu::TextureFormat::R8Unorm,
                )
            });
            if let Some(w) = wheels.iter().find(|w| w.id == id) {
                let mut mask_params = [[0.; 4]; 4];
                mask_params[0][0] = w.opacity_at(time);
                let single = self.film_depths.get(&id).copied();
                if single.is_some() || self.carousel_lenses[0] {
                    let lens=lens::GateLens::for_projector(self.carousel_lenses[0]);
                    let index=order.iter().filter(|id| wheels.iter().any(|w|w.id==**id)).position(|key|*key==id).unwrap_or(0);
                    let spacing=(2*wheel_count.max(1)-1) as f32;
                    let rear=single.unwrap_or(2.*index as f32/spacing);
                    let front=single.unwrap_or((2*index+1) as f32/spacing);
                    mask_params[0][1] = lens.distance(rear);
                    mask_params[0][2] = lens.distance(self.focus.unwrap_or(0.5));
                    mask_params[0][3] = self.detail_scales.get(&id).copied().unwrap_or(1.);
                    mask_params[2] = [lens.distance(front),lens.pupil_scale, f32::from(single.is_none() as u8),0.];
                    if let Some(track) = self.tracks.get(&id) {
                        for (pose,slot) in track.slides.iter().zip([1,3]) {
                            mask_params[slot] = pose.pressure.shader(pose.angle);
                        }
                    }
                }
                self.queue
                    .write_buffer(&g.params, 0, bytemuck::bytes_of(&mask_params));
                self.active = id;
                self.switch_film_sources(w.motion, w.material_motion)
                    .expect("validated film");
                self.slide_visibility = w.films;
                let times = if id == selected {
                    selected_times
                } else {
                    *self.stack_times.get(&id).unwrap_or(&selected_times)
                };
                self.render_one(
                    None,
                    times[0],
                    times[1],
                    time,
                    Some(OpticalExposure::Mask(&g.oil_mask, &mask.view, &g.params)),
                );
            } else if let (Some(m), Some(src)) = (
                scene.tracks.iter().find(|l| l.id == id),
                scene.images.get(&id),
            ) {
                self.draw_media(
                    g,
                    m,
                    src,
                    &self.zero.view,
                    &mask.view,
                    true,
                    time,
                    scene.features,
                );
            }
            g.mattes.insert(id, mask);
        }
        let mut from = 0;
        let mut wheel_index = 0;
        for &id in order {
            let dest = 1 - from;
            if let Some(w) = wheels.iter().find(|w| w.id == id) {
                self.active = id;
                self.wheel.clone_from(&w.wheel);
                self.swatches = w.swatches;
                self.active_material = self.stack_materials[&id];
                self.switch_film_sources(w.motion, w.material_motion)
                    .expect("validated film");
                self.slide_visibility = w.films;
                let times = if id == selected {
                    selected_times
                } else {
                    *self.stack_times.get(&id).unwrap_or(&selected_times)
                };
                let spacing = (2 * wheel_count - 1) as f32;
                let mat = self.exposure_material(
                    -1. - w.opacity_at(time),
                    [
                        (2 * wheel_index) as f32 / spacing,
                        (2 * wheel_index + 1) as f32 / spacing,
                    ],
                );
                self.queue
                    .write_buffer(&self.material_uniform, 0, bytemuck::bytes_of(&mat));
                self.render_one(
                    None,
                    times[0],
                    times[1],
                    time,
                    Some(OpticalExposure::Stack(stack::Exposure {
                        gpu: s,
                        incident: &s.images[from].view,
                        output: &s.images[dest].view,
                    })),
                );
                wheel_index += 1;
            } else if let (Some(m), Some(src)) = (
                scene.tracks.iter().find(|l| l.id == id),
                scene.images.get(&id),
            ) {
                self.draw_media(
                    g,
                    m,
                    src,
                    &s.images[from].view,
                    &s.images[dest].view,
                    false,
                    time,
                    scene.features,
                );
            } else {
                continue;
            }
            from = dest;
        }
        from
    }
    pub(super) fn render_mixed(
        &mut self,
        out: Option<&gpu::TextureView>,
        selected_times: [f64; 2],
        time: f64,
    ) {
        use gpu::BindingResource::{Sampler, TextureView};
        let mut scene = std::mem::take(&mut self.mixed);
        let size = self.preview_size();
        let mut s = self
            .stack_gpu
            .take()
            .unwrap_or_else(|| stack::StackGpu::new(&self.device, size));
        if [s.images[0].texture.width(), s.images[0].texture.height()] != size {
            s.images = stack::StackGpu::images(&self.device, size);
        }
        let mut g = scene
            .gpu
            .take()
            .unwrap_or_else(|| MediaGpu::new(&self.device));
        if g.size != size {
            g.mattes.clear();
            g.size = size;
        }
        g.mattes.retain(|id, _| scene.tracks.iter().any(|l| l.media.mask_a == Some(*id) || l.media.mask_b == Some(*id)));
        let wheels = std::mem::take(&mut self.stack);
        let selected = self.active;
        let active_wheel = std::mem::take(&mut self.wheel);
        let selected_optics = (self.swatches, self.active_material);
        let active_motion = (self.motion, self.material_motion);
        let active_films = self.slide_visibility;
        let split = !self.beams.secondary_ids.is_empty() || self.primary_intensity != 1.;
        let from = if split {
            let (secondary, primary): (Vec<u64>, Vec<u64>) = scene.order.iter().copied().partition(|id| self.beams.secondary_ids.contains(id));
            let mut screen = scene.screen.take().unwrap_or_else(|| beams::ScreenSum::new(&self.device, size));
            if [screen.first.texture.width(), screen.first.texture.height()] != size {
                screen.first = Target::new(&self.device, "First projector exposure", size, FIELD_FORMAT);
            }
            let a = self.render_beam(&scene, &mut g, &s, &wheels, &primary, selected, selected_times, time);
            screen.store_first(self, &s.images[a].view);
            let secondary_beam = self.beams.secondary.filter(|b| b.enabled && b.intensity > 0. && !secondary.is_empty());
            let (b, intensity, offset) = if let Some(beam) = secondary_beam {
                let saved = (self.focus, self.lamp_gain,self.carousel_lenses[0]);
                self.focus = beam.focus;
                self.lamp_gain = self.beams.secondary_gain;
                self.carousel_lenses[0] = self.carousel_lenses[1];
                let index = self.render_beam(&scene, &mut g, &s, &wheels, &secondary, selected, selected_times, time);
                (self.focus, self.lamp_gain,self.carousel_lenses[0]) = saved;
                (index, beam.intensity, beam.offset)
            } else { (a, 0., [0.; 2]) };
            screen.compose(self, &s.images[b].view, &s.images[1-b].view, intensity, offset);
            scene.screen = Some(screen);
            1-b
        } else { self.render_beam(&scene, &mut g, &s, &wheels, &scene.order, selected, selected_times, time) };
        self.active = selected;
        self.wheel = active_wheel;
        (self.swatches, self.active_material) = selected_optics;
        self.switch_film_sources(active_motion.0, active_motion.1)
            .expect("restore selected wheel");
        self.slide_visibility = active_films;
        self.upload_material();
        self.stack = wheels;
        let light = &s.images[from].view;
        let mut enc = self.device.create_command_encoder(&Default::default());
        if let Some(f) = &mut self.feedback {
            f.prepare(&self.device, size);
            f.compose_from(
                &self.device,
                &self.queue,
                &mut enc,
                &self.sampler,
                &self.optics.view,
                time,
                Some(light),
            );
        } else {
            draw(
                &mut enc,
                "Encode mixed projection",
                &self.optics.view,
                &s.encode,
                &bind(
                    &self.device,
                    &s.encode,
                    &[TextureView(light), Sampler(&self.sampler)],
                ),
            );
        }
        if let Some(out) = out {
            draw(
                &mut enc,
                "Present mixed projection",
                out,
                &self.blit_pass,
                &bind(
                    &self.device,
                    &self.blit_pass,
                    &[TextureView(&self.optics.view), Sampler(&self.sampler)],
                ),
            );
        }
        self.queue.submit(Some(enc.finish()));
        self.stack_gpu = Some(s);
        scene.gpu = Some(g);
        self.mixed = scene;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_track::{MaskOp, MediaMode};

    fn track(id: u64) -> MediaTrack {
        let media=serde_json::from_value(serde_json::json!({
            "assetId":id,"durationUs":2_000_000,"still":false,"offsetUs":0,"rate":1,"loop":true,
            "mode":"color","keyColor":[0,1,0],"keyThreshold":0.08,"keySoftness":0.04,
            "gradientA":[1,0,0],"gradientB":[0,0,1],"gradientRate":0.5,
            "bounce":0,"blur":0,"distortion":0,"maskA":null,"maskB":null,"maskOp":"intersect","shape":"none"
        })).unwrap();
        MediaTrack {
            id,
            opacity: 1.,
            envelope: None,
            media,
        }
    }

    fn decoded(id: u64, time: i64) -> Result<DecodedMediaFrame, String> {
        let rgba: Vec<_> = (0..8)
            .flat_map(|p| match id {
                1 => [255, 0, 0, 255],
                2 => [0, 255, 0, 128],
                3 => [255, 0, 255, if p % 4 < 2 { 255 } else { 0 }],
                4 => [234, 42, 90, if p < 4 { 255 } else { 0 }],
                5 if time >= 500_000 => [255, 255, 255, 255],
                5 => [0, 0, 255, 255],
                _ => [0, 0, 0, 0],
            })
            .collect();
        Ok(DecodedMediaFrame {
            id,
            pts_us: time,
            width: 4,
            height: 2,
            rgba: rgba.into(),
        })
    }

    fn frame(r: &mut LiveWheel, tracks: Vec<MediaTrack>, order: Vec<u64>, t: f64) -> Vec<u8> {
        r.set_media_scene(tracks, order).unwrap();
        r.prepare_media(t, true, [0.; 4], decoded).unwrap();
        r.render_program_to(None, t, t);
        r.capture_rgba8().unwrap().2
    }
    fn pixel(image: &[u8], x: usize, y: usize) -> [u8; 3] {
        let index = 4 * (128 * y + x);
        image[index..index + 3].try_into().unwrap()
    }

    #[test]
    fn cropped_media_region_source_clock() {
        let mut l = track(5);
        l.media.offset_us = 100_000;
        let env:OpacityEnvelope=serde_json::from_value(serde_json::json!([
            {"start":3,"end":4,"sourceStart":2,"fadeIn":0,"fadeOut":0,"level":1,"mediaStartUs":620_000,"mediaRate":0.5},
            {"start":7,"end":8,"fadeIn":0,"fadeOut":0,"level":1,"mediaStartUs":620_000,"mediaRate":0.5}
        ])).unwrap();
        for (t, expect) in [
            (2., 100_000),
            (3., 620_000),
            (3.2, 720_000),
            (4., 100_000),
            (7.2, 720_000),
        ] {
            assert_eq!(l.media.source_time(t, Some(&env)), expect);
        }
        l.media.looping = false;
        assert_eq!(l.media.source_time(18., None), 1_999_999);
        l.media.still = true;
        assert_eq!(l.media.source_time(3.2, Some(&env)), 0);
    }

    #[test]
    #[ignore = "requires a reserved discrete GPU selected with TSS_LIVE_ADAPTER"]
    fn projector_lamp_once_per_optical_path() -> Result<(), String> {
        let (mut r, adapter) = crate::coral_capture::discrete_renderer([128, 72])?;
        println!("Lamp readback {} {:?}", adapter.name, adapter.backend);
        let linear = |v: u8| {
            let x = f32::from(v) / 255.;
            if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
        };
        let layers: Vec<_> = (1..=3).map(|id| WheelLayer {
            id,
            wheel: "violet-blue".into(),
            swatches: None,
            motion: MotionPreset::WheelFlow,
            material_motion: Default::default(),
            films: [true; 2],
            opacity: 0.65,
            envelope: None,
        }).collect();
        let mut film = track(8);
        film.opacity = 0.55;
        for path in ["solo", "stack", "media", "mixed"] {
            let oil = match path {
                "stack" | "mixed" => layers.clone(),
                _ => vec![],
            };
            let mut order: Vec<_> = oil.iter().map(|layer| layer.id).collect();
            r.set_stack(oil)?;
            let art = if matches!(path, "media" | "mixed") {
                order.insert(0, 8);
                vec![film.clone()]
            } else {
                vec![]
            };
            r.set_media_scene(art, order)?;
            r.prepare_media(0., true, [0.; 4], |id, _| Ok(DecodedMediaFrame {
                id,
                pts_us: 0,
                width: 1,
                height: 1,
                rgba: Arc::from([140, 140, 140, 255]),
            }))?;
            r.set_lamp_temperature(6500)?;
            r.render_offscreen(0.)?;
            let (_, _, base) = r.capture_rgba8()?;
            for k in [2700, 10000] {
                r.set_lamp_temperature(k)?;
                r.render_offscreen(0.)?;
                let (_, _, frame) = r.capture_rgba8()?;
                if path == "solo" {
                    crate::plates::check_preview_alignment(&mut r, [0.; 2], &frame)?;
                }
                if path == "media" {
                    let mut movie = pollster::block_on(r.begin_movie([128, 72], false))?;
                    movie.frame(0., 0., |pixels| {
                        assert!(pixels == frame, "media-only export changed the projector lamp");
                        Ok(())
                    })?;
                }
                let gain = crate::lamp::linear_gain(k)?;
                let mut residual = [0.; 3];
                let mut count = [0; 3];
                for (a, b) in base.chunks_exact(4).zip(frame.chunks_exact(4)) {
                    for i in 0..3 {
                        let expected = linear(a[i]) * gain[i];
                        if a[i] < 248 && (0.02..0.94).contains(&expected) {
                            residual[i] += (expected - linear(b[i])).abs();
                            count[i] += 1;
                        }
                    }
                }
                let mae: [f32; 3] = std::array::from_fn(|i| residual[i] / count[i] as f32);
                println!("{path} at {k} K: {count:?} samples; linear absolute error {mae:?}");
                assert!(count.iter().all(|n| *n > 100) && mae.iter().all(|e| *e < 0.008));
            }
            r.set_lamp_temperature(6500)?;
            r.render_offscreen(0.)?;
            assert!(base == r.capture_rgba8()?.2, "6500 K restore for {path}");
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires an isolated software Vulkan ICD"]
    fn mixed_artwork_projection_and_movie() {
        let mut r = crate::feedback_test::fixture([128, 72]);
        let red = track(1);
        let mut green = track(2);
        let mut blue = track(5);
        let blend = frame(&mut r, vec![red.clone(), green.clone()], vec![1, 2], 0.);
        let center = pixel(&blend, 64, 32);
        assert!(
            center[0].abs_diff(187) <= 2 && center[1].abs_diff(188) <= 2 && center[2] == 0,
            "linear alpha: {center:?}"
        );
        let reverse = frame(&mut r, vec![red.clone(), green.clone()], vec![2, 1], 0.);
        assert_eq!(pixel(&reverse, 64, 32), [255, 0, 0]);
        green.media.mode = MediaMode::Gradient;
        green.media.key_enabled = true;
        assert_eq!(
            frame(&mut r, vec![red.clone(), green], vec![1, 2], 0.),
            reverse
        );
        let masks = [track(3), track(4)];
        blue.media.mask_a = Some(3);
        blue.media.mask_b = Some(4);
        for (op, expected) in [
            (MaskOp::Intersect, [true, false, false, false]),
            (MaskOp::Union, [true, true, true, false]),
            (MaskOp::Subtract, [false, false, true, false]),
            (MaskOp::Xor, [false, true, true, false]),
        ] {
            blue.media.mask_op = op;
            let img = frame(
                &mut r,
                vec![
                    red.clone(),
                    masks[0].clone(),
                    masks[1].clone(),
                    blue.clone(),
                ],
                vec![1, 5],
                0.,
            );
            for ((x, y), on) in [(12, 8), (114, 8), (12, 55), (114, 55)]
                .into_iter()
                .zip(expected)
            {
                assert_eq!(
                    pixel(&img, x, y),
                    if on { [0, 0, 255] } else { [255, 0, 0] },
                    "{op:?} at {x},{y}"
                );
            }
        }
        blue.media.mask_b = None;
        blue.media.mask_a = None;
        let first = frame(&mut r, vec![blue.clone()], vec![5], 0.);
        let next = frame(&mut r, vec![blue], vec![5], 0.5);
        assert_eq!(pixel(&first, 64, 32), [0, 0, 255]);
        assert_eq!(pixel(&next, 64, 32), [255; 3]);
        {
            let mut movie = pollster::block_on(r.begin_movie([128, 72], false)).unwrap();
            movie.prepare_media(0.5, [0.; 4], decoded).unwrap();
            movie
                .frame(0.5, 0.5, |bytes| {
                    assert!(bytes == next, "preview/movie pixels differ");
                    Ok(())
                })
                .unwrap();
        }
        let mut shaped = track(5);
        shaped.media.mode = MediaMode::Silhouette;
        shaped.media.shape = crate::media_track::Cutout::Star;
        let star = frame(&mut r, vec![shaped.clone()], vec![5], 0.);
        assert_eq!(pixel(&star, 64, 36), [255, 0, 0]);
        assert_eq!(pixel(&star, 16, 10), [0, 0, 0]);
        shaped.media.mode = MediaMode::Gradient;
        let gradient = frame(&mut r, vec![shaped.clone()], vec![5], 0.);
        let cycle = frame(&mut r, vec![shaped.clone()], vec![5], 0.4);
        assert_ne!(pixel(&gradient, 64, 36), pixel(&cycle, 64, 36));
        shaped.media.bounce = 1.;
        shaped.media.blur = 1.;
        shaped.media.distortion = 1.;
        let quiet = frame(&mut r, vec![shaped], vec![5], 0.25);
        r.prepare_media(0.25, true, [1.; 4], decoded).unwrap();
        r.render_program_to(None, 0.25, 0.25);
        let loud = r.capture_rgba8().unwrap().2;
        assert!(
            quiet
                .chunks_exact(4)
                .zip(loud.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count()
                > 500
        );

        let mut tunnel = track(1);
        tunnel.media.mode = MediaMode::Rainbow;
        tunnel.media.still = true;
        tunnel.media.shape = crate::media_track::Cutout::StarTunnel;
        tunnel.media.tunnel_bass = 1.;
        let empty_phase = frame(&mut r, vec![tunnel], vec![1], 0.25);
        r.prepare_media(0.25, true, [0., 0., 0., 0.3], decoded)
            .unwrap();
        r.render_program_to(None, 0.25, 0.25);
        let advanced = r.capture_rgba8().unwrap().2;
        let mut channels = [false; 3];
        for (p, q) in advanced.chunks_exact(4).zip(empty_phase.chunks_exact(4)) {
            if p != q {
                for k in 0..3 {
                    channels[k] |= p[k].abs_diff(q[k]) > 80;
                }
            }
        }
        assert_eq!(
            channels, [true; 3],
            "all rainbow primaries reached the advancing tunnel"
        );
        r.prepare_media(0.25, true, [0.; 4], decoded).unwrap();
        r.render_program_to(None, 0.25, 0.25);
        assert!(
            r.capture_rgba8().unwrap().2 == empty_phase,
            "seek must restore the precise tunnel phase"
        );

        // Optical transmission after opaque artwork alters the artwork. Inverted
        // stack order covers the optical result completely.
        r.set_stack(vec![WheelLayer {
            id: 8,
            wheel: "violet-blue".into(),
            swatches: None,
            motion: MotionPreset::WheelFlow,
            material_motion: MaterialMotion::default(),
            films: [true; 2],
            opacity: 1.,
            envelope: None,
        }])
        .unwrap();
        let under = frame(&mut r, vec![red.clone()], vec![1, 8], 0.);
        let over = frame(&mut r, vec![red.clone()], vec![8, 1], 0.);
        assert!(
            under
                .chunks_exact(4)
                .zip(over.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count()
                > 100
        );
        assert_eq!(pixel(&over, 64, 32), [255, 0, 0]);
        let mut m = red.clone();
        m.media.mask_a = Some(8);
        let masked = frame(&mut r, vec![m.clone()], vec![1], 0.);
        let bright = masked
            .chunks_exact(4)
            .filter(|pixel| pixel[0] > 200)
            .count();
        assert!(
            bright > 200 && bright < 128 * 72 - 200,
            "oil matte coverage {bright}"
        );
        let mut faded = r.stack.clone();
        faded[0].opacity = 0.;
        r.set_stack(faded).unwrap();
        let hidden = frame(&mut r, vec![m], vec![1], 0.);
        assert!(hidden.chunks_exact(4).all(|p| p[..3] == [0; 3]));
    }
}
