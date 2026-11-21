// Offscreen export borrows the renderer exclusively and retains the preview textures and poses.
use super::*;

pub struct WheelMovie<'a> {
    renderer: &'a mut LiveWheel,
    original_optics: Option<Target>,
    original_feedback: Option<feedback::Feedback>,
    original_tracks: HashMap<u64, Track>,
    selected: u64,
    original_times: HashMap<u64, [f64; 2]>,
    current_pose: bool,
    readback: gpu::Buffer,
    row_bytes: u32,
}

// wgpu-core 0.19's Vulkan Maintain::Wait uses a 5 s cleanup wait. Offline
// frames can exceed it. Poll the actual completion callback at both boundaries.
pub(super) fn wait_for_callback<T>(
    device: &gpu::Device,
    recv: std::sync::mpsc::Receiver<T>,
) -> Result<T, String> {
    loop {
        device.poll(gpu::Maintain::Poll);
        match recv.recv_timeout(std::time::Duration::from_millis(2)) {
            Ok(result) => return Ok(result),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
            Err(e) => return Err(e.to_string()),
        }
    }
}

impl LiveWheel {
    pub async fn begin_movie(
        &mut self,
        size: [u32; 2],
        current_pose: bool,
    ) -> Result<WheelMovie<'_>, String> {
        let [w, h] = size;
        check_extent(size, self.device.limits().max_texture_dimension_2d)?;
        let row_bytes = w.checked_mul(4).ok_or("Invalid movie dimensions")?;
        if w == 0 || h == 0 || row_bytes % gpu::COPY_BYTES_PER_ROW_ALIGNMENT != 0 {
            return Err(
                "Movie width must align to 64 RGBA pixels and height must be positive".into(),
            );
        }
        let bytes = u64::from(row_bytes) * u64::from(h);
        if bytes > self.device.limits().max_buffer_size {
            return Err("The selected GPU cannot read back a frame of this size".into());
        }
        let (done, recv) = std::sync::mpsc::channel();
        self.queue.on_submitted_work_done(move || {
            let _ = done.send(());
        });
        wait_for_callback(&self.device, recv)?;
        self.device.push_error_scope(gpu::ErrorFilter::OutOfMemory);
        let optics = Target::new(&self.device, "Movie optical output", size, COLOR_FORMAT);
        let feedback = self
            .feedback
            .as_ref()
            .map(|f| feedback::Feedback::new(&self.device, size, f.settings));
        let readback = self.device.create_buffer(&gpu::BufferDescriptor {
            label: Some("Reusable movie RGBA readback"),
            size: bytes,
            usage: gpu::BufferUsages::COPY_DST | gpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        if let Some(error) = self.device.pop_error_scope().await {
            return Err(error.to_string());
        }
        let original_optics = Some(std::mem::replace(&mut self.optics, optics));
        let original_feedback = std::mem::replace(&mut self.feedback, feedback);
        let selected = self.active;
        let original_times = self.stack_times.clone();
        let original_tracks = std::mem::take(&mut self.tracks);
        for (&id, track) in &original_tracks {
            let mut slides = track.slides;
            for pose in &mut slides {
                if current_pose {
                    pose.end();
                } else {
                    let response = pose.response;
                    *pose = SlidePose::default();
                    pose.response = response;
                }
            }
            let dirty = slides.iter().any(|p| p.strength != [0.; 2]);
            self.tracks.insert(
                id,
                Track {
                    slides,
                    dirty,
                    warp: None,
                },
            );
        }
        Ok(WheelMovie {
            renderer: self,
            original_optics,
            original_feedback,
            original_tracks,
            selected,
            original_times,
            current_pose,
            readback,
            row_bytes,
        })
    }
}

impl WheelMovie<'_> {
    pub fn prepare_media(
        &mut self,
        time: f64,
        features: [f32; 4],
        decode: impl FnMut(u64, i64) -> Result<crate::DecodedMediaFrame, String>,
    ) -> Result<(), String> {
        self.renderer.prepare_media(time, false, features, decode)
    }
    pub fn select_gesture_layer(&mut self, id: u64) -> Result<(), String> {
        if id != self.selected && !self.renderer.stack.iter().any(|l| l.id == id) {
            return Err("Movie gesture targets an unknown wheel".into());
        }
        self.renderer.select_track(id);
        Ok(())
    }
    pub fn set_slide_pressure(
        &mut self,
        slide: u32,
        amount: f32,
        center: [f32; 2],
    ) -> Result<(), String> {
        self.renderer.set_slide_pressure(slide, amount, center)
    }
    pub fn end_gesture(&mut self, slide: u32) {
        self.renderer.end_gesture(slide);
    }

    pub fn gesture(
        &mut self,
        slide: u32,
        mode: SlideGesture,
        response: f32,
        from: [f32; 2],
        to: [f32; 2],
    ) -> Result<(), String> {
        self.renderer.set_tug_response(slide, response)?;
        self.renderer.gesture(slide, mode, from, to);
        Ok(())
    }

    /// The sink sees tightly packed, top-down sRGB RGBA8 until it returns.
    pub fn frame(
        &mut self,
        field_time: f64,
        song_time: f64,
        sink: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.frame_films(field_time, field_time, song_time, sink)
    }
    pub fn frame_films(
        &mut self,
        rear: f64,
        front: f64,
        song_time: f64,
        sink: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        if !rear.is_finite() || !front.is_finite() || !song_time.is_finite() {
            return Err("Invalid movie sample time".into());
        }
        let r = &mut self.renderer;
        let [w, h] = r.preview_size();
        r.select_track(self.selected);
        for layer in &r.stack {
            let origin = if self.current_pose {
                self.original_times
                    .get(&layer.id)
                    .copied()
                    .unwrap_or([0.; 2])
            } else {
                [0.; 2]
            };
            r.stack_times
                .insert(layer.id, [origin[0] + song_time, origin[1] + song_time]);
        }
        r.render_films_to(None, rear, front, song_time);
        let mut encoder = r.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            r.optics.texture.as_image_copy(),
            gpu::ImageCopyBuffer {
                buffer: &self.readback,
                layout: gpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row_bytes),
                    rows_per_image: Some(h),
                },
            },
            extent([w, h], 1),
        );
        r.queue.submit(Some(encoder.finish()));
        let slice = self.readback.slice(..);
        let (send, recv) = std::sync::mpsc::channel();
        slice.map_async(gpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
        wait_for_callback(&r.device, recv)?.map_err(|e| e.to_string())?;
        let result = sink(&slice.get_mapped_range());
        self.readback.unmap();
        result
    }
}

impl Drop for WheelMovie<'_> {
    fn drop(&mut self) {
        let r = &mut self.renderer;
        r.optics = self.original_optics.take().unwrap();
        r.feedback = self.original_feedback.take();
        r.active = self.selected;
        r.stack_times = std::mem::take(&mut self.original_times);
        r.tracks = std::mem::take(&mut self.original_tracks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an isolated software Vulkan ICD"]
    fn movie_feedback_starts_empty_and_restores_the_live_echo() {
        let size = [128, 72];
        let mut r = crate::feedback_test::fixture(size);
        r.set_feedback(Some(FeedbackSettings {
            delay_ms: 50,
            strength: 0.72,
            zoom: 1.03,
            rotation_degrees: 3.,
        }))
        .unwrap();
        let frame = |r: &mut LiveWheel, field, pts| {
            r.render_program_to(None, field, pts);
            r.capture_rgba8().unwrap().2
        };
        let empty = frame(&mut r, 2.25, 0.);
        r.reset_feedback_history();
        frame(&mut r, 0.1, 0.);
        frame(&mut r, 1.1, 0.06);
        let live = frame(&mut r, 2.25, 0.12);
        assert_ne!(live, empty);
        r.reset_feedback_history();
        frame(&mut r, 0.1, 0.);
        frame(&mut r, 1.1, 0.06);
        let mut movie = pollster::block_on(r.begin_movie(size, true)).unwrap();
        movie
            .frame(2.25, 0., |pixels| {
                assert_eq!(pixels, empty.as_slice());
                Ok(())
            })
            .unwrap();
        movie
            .frame(2.25, 0.06, |pixels| {
                assert_ne!(pixels, empty.as_slice());
                Ok(())
            })
            .unwrap();
        drop(movie);
        assert_eq!(frame(&mut r, 2.25, 0.12), live);
    }
}
