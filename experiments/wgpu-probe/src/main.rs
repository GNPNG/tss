use std::{env, sync::mpsc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend_arg = env::args().nth(1).unwrap_or_else(|| "vulkan".into());
    let backends = match backend_arg.as_str() {
        "vulkan" => wgpu::Backends::VULKAN,
        "dx12" => wgpu::Backends::DX12,
        "metal" => wgpu::Backends::METAL,
        _ => return Err("Usage: tss-gpu-compute-probe [vulkan|dx12|metal] [adapter-name-substring]".into()),
    };
    let filter = env::args().nth(2).map(|s| s.to_lowercase());
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..Default::default() });
    let adapters = instance.enumerate_adapters(backends);
    for (i, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        println!("adapter[{i}]: name={:?}, backend={:?}, type={:?}, vendor={:#06x}, device={:#06x}", info.name, info.backend, info.device_type, info.vendor, info.device);
    }
    let adapter = adapters.into_iter().find(|adapter| {
        let info = adapter.get_info();
        match &filter {
            Some(needle) => info.name.to_lowercase().contains(needle),
            None => info.device_type == wgpu::DeviceType::DiscreteGpu,
        }
    }).ok_or("No matching adapter; pass an adapter name substring")?;
    println!("selected: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("TSS GPU probe"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
    }, None))?;
    const WORDS: u64 = 64;
    let storage = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("compute output"), size: WORDS * 4,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"), size: WORDS * 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("probe shader"), source: wgpu::ShaderSource::Wgsl(r#"
            @group(0) @binding(0) var<storage, read_write> data: array<u32>;
            @compute @workgroup_size(64)
            fn main(@builtin(global_invocation_id) id: vec3<u32>) {
                data[id.x] = id.x * 3u + 7u;
            }
        "#.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("probe pipeline"), layout: None, module: &shader, entry_point: "main",
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("probe bindings"), layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: storage.as_entire_binding() }],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("probe commands") });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("probe pass"), timestamp_writes: None });
        pass.set_pipeline(&pipeline); pass.set_bind_group(0, &group, &[]); pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&storage, 0, &readback, 0, WORDS * 4);
    queue.submit(Some(encoder.finish()));
    let (sender, receiver) = mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |result| { let _ = sender.send(result); });
    device.poll(wgpu::Maintain::Wait);
    receiver.recv()??;
    let mapped = readback.slice(..).get_mapped_range();
    for (i, bytes) in mapped.chunks_exact(4).enumerate() {
        let actual = u32::from_le_bytes(bytes.try_into()?);
        let expected = i as u32 * 3 + 7;
        if actual != expected { return Err(format!("Mismatch at {i}: {actual} != {expected}").into()); }
    }
    drop(mapped); readback.unmap();
    println!("PASS: {WORDS} GPU-computed words matched their expected values");
    Ok(())
}
