use std::{env, error::Error, num::NonZeroUsize, sync::{mpsc, Arc}};
use vello::{kurbo::Affine, peniko::{Blob, Color, Format, Image}, AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

const WIDTH: u32 = 512;
const HEIGHT: u32 = 512;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        return Err("usage: tss-native-slide-study <vulkan|dx12|metal> <adapter-name> <output.png> <mask.ppm> [mask.ppm ...]".into());
    }
    let backend = match args[1].as_str() {
        "vulkan" => wgpu::Backends::VULKAN,
        "dx12" => wgpu::Backends::DX12,
        "metal" => wgpu::Backends::METAL,
        _ => return Err("unsupported backend".into()),
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: backend, ..Default::default() });
    let needle = args[2].to_lowercase();
    let adapter = instance.enumerate_adapters(backend).into_iter().find(|a| a.get_info().name.to_lowercase().contains(&needle)).ok_or("adapter not found")?;
    let info = adapter.get_info();
    println!("selected adapter: {info:?}");
    if info.device_type == wgpu::DeviceType::Cpu { return Err("software adapter refused".into()); }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("TSS native slide composition study"), required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::default(),
    }, None))?;
    let mut renderer = Renderer::new(&device, RendererOptions { surface_format: None, use_cpu: false, antialiasing_support: AaSupport::all(), num_init_threads: NonZeroUsize::new(1) })?;
    let mut scene = Scene::new();
    let colors = [[20u8, 185, 205], [190, 70, 170], [65, 105, 210], [220, 125, 30]];
    for (i, path) in args[4..].iter().enumerate() {
        let mask = image::open(path)?.to_luma8();
        let (w, h) = mask.dimensions();
        if w == 0 || h == 0 { return Err("empty mask".into()); }
        let color = colors[i % colors.len()];
        let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
        for px in mask.pixels() { rgba.extend_from_slice(&[color[0], color[1], color[2], px[0]]); }
        let img = Image::new(Blob::new(Arc::new(rgba)), Format::Rgba8, w, h);
        scene.draw_image(&img, Affine::scale_non_uniform(WIDTH as f64 / w as f64, HEIGHT as f64 / h as f64));
        println!("layer {i}: source={path:?}, size={w}x{h}");
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("native slide output"), size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    renderer.render_to_texture(&device, &queue, &scene, &texture.create_view(&Default::default()), &RenderParams {
        base_color: Color::WHITE, width: WIDTH, height: HEIGHT, antialiasing_method: AaConfig::Area,
    })?;
    let bytes_per_row = WIDTH * 4; // WIDTH is chosen so rows satisfy wgpu copy alignment.
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("native slide readback"), size: (bytes_per_row * HEIGHT) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
    encoder.copy_texture_to_buffer(texture.as_image_copy(), wgpu::ImageCopyBuffer {
        buffer: &buffer, layout: wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: Some(HEIGHT) },
    }, texture.size());
    queue.submit(Some(encoder.finish()));
    let (sender, receiver) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| { let _ = sender.send(r); });
    device.poll(wgpu::Maintain::Wait);
    receiver.recv()??;
    let bytes = buffer.slice(..).get_mapped_range();
    image::save_buffer(&args[3], &bytes, WIDTH, HEIGHT, image::ColorType::Rgba8)?;
    drop(bytes);
    buffer.unmap();
    println!("wrote {}x{} image to {:?}", WIDTH, HEIGHT, args[3]);
    Ok(())
}
