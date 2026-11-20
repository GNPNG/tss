#[path = "../material.rs"]
mod material;
#[path = "../tiles.rs"]
mod tiles;
use std::{env, fs, io::Write, path::PathBuf, process::{Command, Stdio}, sync::mpsc};
use wgpu::util::DeviceExt;

fn color_absorption(s: &str) -> Result<[f32; 4], Box<dyn std::error::Error>> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.bytes().all(|c| c.is_ascii_hexdigit()) { return Err("color must be RRGGBB".into()); }
    let mut result = [0.0f32; 4];
    for i in 0..3 {
        let value = u8::from_str_radix(&s[i * 2..i * 2 + 2],16)? as f32 / 255.0;
        let linear = if value <= 0.04045 { value / 12.92 } else { ((value + 0.055)/1.055).powf(2.4) };
        result[i] = -linear.max(0.0001).ln();
    }
    Ok(result)
}

fn save_still(rgba: &[u8], width: u32, height: u32, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut ffmpeg = Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-n", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{width}x{height}"), "-i", "pipe:0", "-frames:v", "1"]).arg(path).stdin(Stdio::piped()).spawn()?;
    ffmpeg.stdin.take().ok_or("no ffmpeg input")?.write_all(rgba)?;
    if !ffmpeg.wait()?.success() { return Err("PNG encoder failed".into()); }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 11 {
        return Err("Usage: physical-optics-study <metal|vulkan|dx12> <fields.f32le> <grid_width> <grid_height> <layers> <frames> <output_width> <output_height> <fps> <new_output_dir> [color0 .. color3] [inversion_bitmask] [blur_px0 .. blur_px3]".into());
    }
    let backend = match args[1].as_str() {
        "metal" => wgpu::Backends::METAL, "vulkan" => wgpu::Backends::VULKAN, "dx12" => wgpu::Backends::DX12,
        _ => return Err("backend must be metal, vulkan or dx12".into()),
    };
    let grid_w: u32 = args[3].parse()?; let grid_h: u32 = args[4].parse()?;
    let layers: u32 = args[5].parse()?; let frames: u32 = args[6].parse()?;
    let width: u32 = args[7].parse()?; let height: u32 = args[8].parse()?; let fps: u32 = args[9].parse()?;
    if grid_w < 2 || grid_h < 2 || layers == 0 || layers > 2 || frames == 0 || width == 0 || height == 0 || fps == 0 { return Err("invalid dimensions or layer count: this study supports one or two two-component vessels".into()); }
    let requested_rows = env::var("TSS_TILE_ROWS").ok().map(|x| x.parse::<u32>()).transpose()?;
    let tiles = tiles::plan(height, requested_rows)?;
    let report_tile_times = env::var("TSS_REPORT_TILE_TIMES").is_ok_and(|v| v == "1");
    let output_dir = PathBuf::from(&args[10]);
    if output_dir.exists() { return Err("output directory already exists".into()); }
    let source = fs::read(&args[2])?;
    let field_bytes = (grid_w as usize).checked_mul(grid_h as usize).and_then(|v|v.checked_mul(layers as usize)).and_then(|v|v.checked_mul(20)).ok_or("field size overflow")?;
    let total_bytes = field_bytes.checked_mul(frames as usize).ok_or("input size overflow")?;
    if source.len() != total_bytes { return Err(format!("expected {total_bytes} input bytes, got {}",source.len()).into()); }
    if source.chunks_exact(4).any(|b| !f32::from_le_bytes(b.try_into().unwrap()).is_finite()) { return Err("all interface distance fields must be finite".into()); }
    let strengths_setting = env::var("TSS_DYE_STRENGTHS").ok();
    let strengths = material::dye_strengths(strengths_setting.as_deref())?;
    let defaults = ["e381c1", "01bdd1", "1746bb", "a25cba"];
    let mut params = Vec::with_capacity(96);
    let inversion_bits: u32 = args.get(15).map(|x| x.parse()).transpose()?.unwrap_or(1);
    let debug_mode:u32=args.get(24).map(|x| x.parse()).transpose()?.unwrap_or(0);
    for v in [width,height,grid_w,grid_h,layers,inversion_bits,debug_mode,0] { params.extend(v.to_le_bytes()); }
    for (i, default) in defaults.iter().enumerate() {
        let mut absorption=color_absorption(args.get(11+i).map(String::as_str).unwrap_or(default))?;
        for channel in &mut absorption[0..3] { *channel *= strengths[i]; }
        let blur: f32=args.get(16+i).map(|x| x.parse()).transpose()?.unwrap_or([2.0,1.0,1.5,2.0][i]);
        if !blur.is_finite() || !(0.0..=64.0).contains(&blur) { return Err("blur radii must be finite, 0..64 pixels".into()); }
        absorption[3]=blur;
        for v in absorption { params.extend(v.to_le_bytes()); }
    }
    // Geometric gap, clear index, dyed index, pupil angular slope. These are
    // explicit scene assumptions, not measurements of the reference device.
    let optical_defaults=[0.07_f32,1.333,1.46,0.12];
    for (i, default) in optical_defaults.iter().enumerate() {
        let value: f32=args.get(20+i).map(|x| x.parse()).transpose()?.unwrap_or(*default);
        if !value.is_finite() || value <= 0.0 { return Err("optical parameters must be finite and positive".into()); }
        params.extend(value.to_le_bytes());
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor{backends:backend,..Default::default()});
    let adapters = instance.enumerate_adapters(backend);
    let adapter = adapters.iter().find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu).or_else(||adapters.first()).ok_or("no GPU adapter")?;
    println!("optical study GPU: {:?}", adapter.get_info());
    let (device,queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("optical study"), required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::downlevel_defaults() },None))?;
    let bytes = u64::from(width).checked_mul(u64::from(height)).and_then(|x|x.checked_mul(4)).ok_or("output size overflow")?;
    let field_buffer=device.create_buffer(&wgpu::BufferDescriptor{label:Some("independent solver fields"),size:field_bytes as u64,usage:wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
    let out=device.create_buffer(&wgpu::BufferDescriptor{label:Some("display-referred rgba"),size:bytes,usage:wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_SRC|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
    let readback=device.create_buffer(&wgpu::BufferDescriptor{label:Some("frame readback"),size:bytes,usage:wgpu::BufferUsages::MAP_READ|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
    let uniform=device.create_buffer_init(&wgpu::util::BufferInitDescriptor{label:Some("optics parameters"),contents:&params,usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST});
    let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{label:Some("layer optics"),source:wgpu::ShaderSource::Wgsl(include_str!("../tracer.wgsl").into())});
    let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor{label:Some("transmitted light"),layout:None,module:&shader,entry_point:"main"});
    let group=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("inputs"),layout:&pipeline.get_bind_group_layout(0),entries:&[
        wgpu::BindGroupEntry{binding:0,resource:field_buffer.as_entire_binding()},wgpu::BindGroupEntry{binding:1,resource:out.as_entire_binding()},wgpu::BindGroupEntry{binding:2,resource:uniform.as_entire_binding()},]});
    fs::create_dir(&output_dir)?;
    let mut video=Command::new("ffmpeg").args(["-hide_banner","-loglevel","error","-n","-f","rawvideo","-pix_fmt","rgba","-s",&format!("{width}x{height}"),"-r",&fps.to_string(),"-i","pipe:0","-an","-c:v","libx264","-pix_fmt","yuv420p"]).arg(output_dir.join("study.mp4")).stdin(Stdio::piped()).spawn()?;
    let mut input=video.stdin.take().ok_or("no ffmpeg input")?;
    for frame in 0..frames as usize {
        queue.write_buffer(&field_buffer,0,&source[frame*field_bytes..(frame+1)*field_bytes]);
        // Clearing prevents a failed dispatch from returning opaque pixels
        // left over from a previous frame. Each row batch has its own submit.
        let mut clear=device.create_command_encoder(&wgpu::CommandEncoderDescriptor{label:Some("clear frame")});
        clear.clear_buffer(&out,0,None);
        queue.submit(Some(clear.finish()));
        for tile in &tiles {
            queue.write_buffer(&uniform,28,&tile.start.to_le_bytes());
            device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut encoder=device.create_command_encoder(&wgpu::CommandEncoderDescriptor{label:Some("bounded row dispatch")});
            { let mut pass=encoder.begin_compute_pass(&wgpu::ComputePassDescriptor{label:Some("optical pass"),timestamp_writes:None}); pass.set_pipeline(&pipeline); pass.set_bind_group(0,&group,&[]); pass.dispatch_workgroups(width.div_ceil(8),(tile.end-tile.start).div_ceil(8),1); }
            let submitted_at=std::time::Instant::now();
            queue.submit(Some(encoder.finish()));
            device.poll(wgpu::Maintain::Wait);
            if report_tile_times {
                eprintln!("host_submit_wait frame={frame} rows={}..{} elapsed_ms={:.3}",tile.start,tile.end,submitted_at.elapsed().as_secs_f64()*1000.0);
            }
            if let Some(e)=pollster::block_on(device.pop_error_scope()) { return Err(format!("GPU validation error in rows {}..{}: {e}",tile.start,tile.end).into()); }
            if let Some(e)=pollster::block_on(device.pop_error_scope()) { return Err(format!("GPU out-of-memory error in rows {}..{}: {e}",tile.start,tile.end).into()); }
        }
        let mut encoder=device.create_command_encoder(&wgpu::CommandEncoderDescriptor{label:Some("frame readback")});
        encoder.copy_buffer_to_buffer(&out,0,&readback,0,bytes); queue.submit(Some(encoder.finish()));
        let (tx,rx)=mpsc::channel(); readback.slice(..).map_async(wgpu::MapMode::Read,move |r|{let _=tx.send(r);}); device.poll(wgpu::Maintain::Wait); rx.recv()??;
        let mapped=readback.slice(..).get_mapped_range();
        // Every invocation writes opaque alpha. An all-zero or partially
        // unwritten GPU buffer is a failed render, not a valid black frame.
        if let Some((pixel, _)) = mapped.chunks_exact(4).enumerate().find(|(_, px)| px[3] != 255) {
            return Err(format!("GPU readback missing opaque output at frame {frame}, pixel {pixel}").into());
        }
        input.write_all(&mapped)?;
        if frame == 0 || frame == frames as usize / 2 || frame + 1 == frames as usize { save_still(&mapped,width,height,&output_dir.join(format!("frame-{frame:04}.png")))?; }
        drop(mapped);readback.unmap();
        if frame % 30 == 0 { println!("frame {}/{}",frame+1,frames); }
    }
    drop(input); if !video.wait()?.success() { return Err("video encoder failed".into()); }
    Ok(())
}
