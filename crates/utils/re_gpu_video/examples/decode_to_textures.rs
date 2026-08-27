//! Decodes an H.264 elementary stream (annex-b) through the texture-output decoder,
//! samples every frame's NV12 plane views in a render pass, and compares the result
//! byte for byte against the CPU readback decoder:
//!
//! ```sh
//! cargo run -p re_gpu_video --example decode_to_textures -- tests/assets/ipb.h264
//! ```
//!
//! Runs with wgpu validation on, so it also verifies that the wrapped textures are
//! acceptable to wgpu as sampling sources. Exits non-zero on any mismatch.

use re_gpu_video::{CpuFrame, DecodedFrame, VideoDeviceSetup};

/// Renders vec4(y, u, v, 1) per pixel from the two plane views, by texel load.
const SHADER: &str = "
@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    let pos = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
    return vec4f(pos[index], 0.0, 1.0);
}

@group(0) @binding(0) var y_tex: texture_2d<f32>;
@group(0) @binding(1) var uv_tex: texture_2d<f32>;

@fragment fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let p = vec2i(pos.xy);
    let y = textureLoad(y_tex, p, 0).r;
    let uv = textureLoad(uv_tex, p / 2, 0).rg;
    return vec4f(y, uv, 1.0);
}
";

fn main() {
    re_log::setup_logging();

    let mut args = std::env::args().skip(1);
    let Some(input) = args.next() else {
        eprintln!("Usage: decode_to_textures <input.h264>");
        std::process::exit(1);
    };

    let data = std::fs::read(&input).expect("failed to read the input file");

    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_descriptor.flags |= wgpu::InstanceFlags::VALIDATION;
    let instance = wgpu::Instance::new(instance_descriptor);
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));

    let Some((adapter, mut setup)) = adapters
        .iter()
        .find_map(|adapter| VideoDeviceSetup::request(adapter).map(|setup| (adapter, setup)))
    else {
        eprintln!("No adapter with H.264 decode support found.");
        std::process::exit(1);
    };
    println!("Decoding on {}", adapter.get_info().name);

    let descriptor = wgpu::DeviceDescriptor {
        label: Some("decode_to_textures"),
        required_features: adapter
            .features()
            .intersection(wgpu::Features::TEXTURE_FORMAT_NV12),
        ..Default::default()
    };
    let (device, queue) = if setup.needs_hal_device_creation() {
        #[expect(unsafe_code)]
        // SAFETY: Mirrors what `re_renderer::device::create_device` does:
        // the hal device is created from this adapter and handed straight to wgpu.
        unsafe {
            let hal_adapter = adapter
                .as_hal::<wgpu::hal::api::Vulkan>()
                .expect("probed adapter must be a Vulkan adapter");
            let open_device = hal_adapter
                .open_with_callback(
                    descriptor.required_features,
                    &descriptor.required_limits,
                    &descriptor.memory_hints,
                    Some(setup.create_device_callback()),
                )
                .expect("hal device creation failed");
            adapter
                .create_device_from_hal(open_device, &descriptor)
                .expect("wgpu device creation failed")
        }
    } else {
        pollster::block_on(adapter.request_device(&descriptor))
            .expect("wgpu device creation failed")
    };

    let context = setup
        .into_context(&device)
        .expect("video context creation failed");

    // Decode the stream twice: through the texture path under test,
    // and through the CPU readback path as the reference.
    let mut texture_decoder = context
        .create_h264_decoder()
        .expect("decoder creation failed");
    let mut texture_frames = texture_decoder
        .push_access_unit(&data, 0)
        .expect("decoding failed");
    texture_frames.extend(texture_decoder.flush().expect("flush failed"));

    let mut cpu_decoder = context
        .create_h264_cpu_decoder()
        .expect("decoder creation failed");
    let cpu_frames = cpu_decoder
        .push_access_unit(&data)
        .expect("decoding failed");

    // The CPU path emits decode order: sort by picture order count within the
    // groups delimited by IDR frames, matching the texture path's output order.
    let mut reference: Vec<CpuFrame> = Vec::new();
    let mut pending: Vec<CpuFrame> = Vec::new();
    for frame in cpu_frames {
        if frame.is_idr {
            pending.sort_by_key(|frame| frame.poc);
            reference.append(&mut pending);
        }
        pending.push(frame);
    }
    pending.sort_by_key(|frame| frame.poc);
    reference.append(&mut pending);

    assert_eq!(
        texture_frames.len(),
        reference.len(),
        "the two decoders produced different frame counts"
    );

    let sampler = PlaneSampler::new(&device);
    let mut failed = 0;
    for (index, (frame, reference)) in std::iter::zip(&texture_frames, &reference).enumerate() {
        assert_eq!(
            (frame.width, frame.height),
            (reference.width, reference.height)
        );
        let yuv = sampler.sample(&device, &queue, frame);
        let mismatches = compare(&yuv, reference);
        if mismatches > 0 {
            eprintln!("Frame {index}: {mismatches} mismatched samples");
            failed += 1;
        }
    }

    if failed > 0 {
        eprintln!("FAILED: {failed} of {} frames mismatched", reference.len());
        std::process::exit(1);
    }
    println!(
        "OK: {} frames match the CPU readback path exactly",
        reference.len()
    );
}

/// Renders a frame's plane views into an RGBA target and reads it back,
/// returning tightly packed rows of (y, u, v, 255) samples.
struct PlaneSampler {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl PlaneSampler {
    fn new(device: &wgpu::Device) -> Self {
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[texture_entry(0), texture_entry(1)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout }
    }

    fn sample(&self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &DecodedFrame) -> Vec<u8> {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&frame.y),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&frame.uv),
                },
            ],
        });

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&Default::default());

        let bytes_per_row = (frame.width * 4).next_multiple_of(256);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(bytes_per_row) * u64::from(frame.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);

        readback.map_async(wgpu::MapMode::Read, .., |result| {
            result.expect("readback mapping failed");
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("device lost");

        let mapped = readback
            .get_mapped_range(..)
            .expect("readback mapping failed");
        let mut rows = Vec::with_capacity((frame.width * 4 * frame.height) as usize);
        for row in 0..frame.height {
            let start = (row * bytes_per_row) as usize;
            rows.extend_from_slice(&mapped[start..start + (frame.width * 4) as usize]);
        }
        rows
    }
}

/// Counts samples where the rendered (y, u, v) bytes differ from the CPU frame.
fn compare(yuv: &[u8], reference: &CpuFrame) -> usize {
    let width = reference.width as usize;
    let height = reference.height as usize;
    let luma_size = width * height;
    let (luma, chroma) = reference.data.split_at(luma_size);
    let chroma_stride = width.div_ceil(2) * 2;

    let mut mismatches = 0;
    for y in 0..height {
        for x in 0..width {
            let rendered = &yuv[(y * width + x) * 4..][..3];
            let expected_y = luma[y * width + x];
            let expected_uv = &chroma[(y / 2) * chroma_stride + (x / 2) * 2..][..2];
            if rendered != [expected_y, expected_uv[0], expected_uv[1]] {
                mismatches += 1;
            }
        }
    }
    mismatches
}
