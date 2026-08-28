//! End to end tests over the real `VideoToolbox` decoder and the Metal interop.
//!
//! Unlike Vulkan Video, `VideoToolbox` decodes in software wherever the hardware
//! can't, so these run on CI machines too. They create a Metal device directly
//! rather than going through whatever adapter the rest of the test suite picks.

use std::sync::Arc;

use crate::vulkan::{h264, h265};
use crate::{Codec, DecodedFrame, GpuVideoContext, VideoDeviceSetup};

/// A Metal device with a video context, `None` when the machine has no Metal adapter.
fn metal_context() -> Option<(wgpu::Device, wgpu::Queue, Arc<GpuVideoContext>)> {
    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_descriptor.backends = wgpu::Backends::METAL;
    instance_descriptor.flags |= wgpu::InstanceFlags::VALIDATION;
    let instance = wgpu::Instance::new(instance_descriptor);

    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::METAL));
    let adapter = adapters.into_iter().next()?;

    let setup = VideoDeviceSetup::request(&adapter).expect("Metal always has VideoToolbox");
    assert!(
        !setup.needs_hal_device_creation(),
        "the VideoToolbox backend works against a plainly created device"
    );

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("videotoolbox tests"),
        ..Default::default()
    }))
    .expect("device creation failed");

    let context = setup
        .into_context(&device)
        .expect("video context creation failed");
    Some((device, queue, context))
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/assets/{name}", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(&path).expect("fixture missing, run tests/assets/generate.sh");
    assert!(
        data.len() > 100,
        "Fixture is a stub, git-lfs checkout needed.\nFile path: {path}"
    );
    data
}

/// Splits an elementary stream on the access unit delimiters the fixtures are
/// generated with: NAL unit type 9 for H.264, 35 for H.265.
fn split_on_aud(codec: Codec, data: &[u8]) -> Vec<&[u8]> {
    let delimiter = match codec {
        Codec::H264 => 9,
        Codec::H265 => 35 << 1,
        Codec::AV1 => panic!("VideoToolbox decodes no AV1 here"),
    };
    let cuts: Vec<usize> = data
        .windows(4)
        .enumerate()
        .filter_map(|(index, window)| (window == [0, 0, 1, delimiter]).then_some(index))
        .collect();
    assert!(!cuts.is_empty(), "fixture has no access unit delimiters");

    cuts.iter()
        .enumerate()
        .map(|(index, &cut)| {
            let end = cuts.get(index + 1).copied().unwrap_or(data.len());
            &data[cut..end]
        })
        .collect()
}

/// The presentation order key of each access unit, from this crate's own parsers.
///
/// The decoder passes timestamps through untouched and sorts by them, so feeding
/// the picture order counts is what puts a stream with B frames back in order.
fn picture_order_counts(codec: Codec, units: &[&[u8]]) -> Vec<i64> {
    let mut counts = Vec::new();
    match codec {
        Codec::H264 => {
            let mut parser = h264::Parser::new(16);
            for unit in units {
                for op in parser.push_access_unit(unit).expect("parsing failed") {
                    if let h264::DecodeOp::DecodeFrame(info) = op {
                        counts.push(i64::from(info.poc));
                    }
                }
            }
        }
        Codec::H265 => {
            let mut parser = h265::Parser::new(16);
            for unit in units {
                for op in parser.push_access_unit(unit).expect("parsing failed") {
                    if let h265::DecodeOp::DecodeFrame(info) = op {
                        counts.push(i64::from(info.poc));
                    }
                }
            }
        }
        Codec::AV1 => panic!("VideoToolbox decodes no AV1 here"),
    }
    counts
}

/// Decodes a whole fixture, one push per access unit.
fn decode_fixture(context: &GpuVideoContext, codec: Codec, name: &str) -> Vec<DecodedFrame> {
    let data = fixture(name);
    let units = split_on_aud(codec, &data);
    let timestamps = picture_order_counts(codec, &units);
    assert_eq!(units.len(), timestamps.len());

    let mut decoder = context
        .create_decoder(codec)
        .expect("decoder creation failed");
    let mut frames = Vec::new();
    for (unit, pts) in units.iter().zip(timestamps) {
        frames.extend(
            decoder
                .push_access_unit(unit, pts)
                .expect("decoding failed"),
        );
    }
    frames.extend(decoder.flush().expect("flush failed"));
    frames
}

/// A stream without reordering decodes to one frame per access unit, of the
/// declared size, and the frames come out as different pictures rather than the
/// same surface over and over.
#[test]
fn decodes_every_frame_to_its_own_texture() {
    let Some((device, queue, context)) = metal_context() else {
        return;
    };

    let frames = decode_fixture(&context, Codec::H264, "ippp.h264");
    assert_eq!(frames.len(), 16);

    let sampler = PlaneSampler::new(&device);
    let mut previous: Option<Vec<u8>> = None;
    for (index, frame) in frames.iter().enumerate() {
        assert_eq!((frame.width, frame.height), (64, 64));
        assert_eq!(frame.pts, i64::try_from(index).unwrap() * 2);
        assert_eq!(frame.is_idr, index == 0);

        let luma = sampler.sample_luma(&device, &queue, frame);
        assert!(
            luma.iter().any(|&sample| sample != luma[0]),
            "frame {index} is a flat surface, the plane view holds no picture"
        );
        if let Some(previous) = &previous {
            assert_ne!(
                previous, &luma,
                "frame {index} is the same picture as the one before it, \
                 the pixel buffers are being recycled while still in use"
            );
        }
        previous = Some(luma);
    }
}

/// A stream with B frames arrives in presentation order, not decode order.
#[test]
fn reorders_b_frames_into_presentation_order() {
    let Some((_device, _queue, context)) = metal_context() else {
        return;
    };

    let frames = decode_fixture(&context, Codec::H264, "ipb.h264");
    assert_eq!(frames.len(), 16);

    let timestamps: Vec<i64> = frames.iter().map(|frame| frame.pts).collect();
    let mut sorted = timestamps.clone();
    sorted.sort_unstable();
    assert_eq!(
        timestamps, sorted,
        "the decoder emitted frames out of presentation order"
    );
}

/// H.265 goes down the same path, only the parameter sets and the NAL unit
/// header differ.
#[test]
fn decodes_h265() {
    let Some((_device, _queue, context)) = metal_context() else {
        return;
    };

    let frames = decode_fixture(&context, Codec::H265, "ipb.h265");
    assert_eq!(frames.len(), 16);

    let timestamps: Vec<i64> = frames.iter().map(|frame| frame.pts).collect();
    let mut sorted = timestamps.clone();
    sorted.sort_unstable();
    assert_eq!(timestamps, sorted);
    assert!(
        frames
            .iter()
            .all(|frame| (frame.width, frame.height) == (64, 64))
    );
}

/// A resolution change mid-stream rebuilds the session and keeps decoding.
#[test]
fn follows_a_mid_stream_resolution_change() {
    let Some((_device, _queue, context)) = metal_context() else {
        return;
    };

    let frames = decode_fixture(&context, Codec::H264, "sps_change.h264");
    assert_eq!(frames.len(), 12);
    assert_eq!(
        frames
            .iter()
            .map(|frame| (frame.width, frame.height))
            .collect::<Vec<_>>(),
        [(64, 64); 6]
            .into_iter()
            .chain([(96, 64); 6])
            .collect::<Vec<_>>()
    );
}

/// Renders a frame's luma plane into a target texture and reads it back,
/// so that the tests look at what wgpu actually sees in the wrapped textures.
struct PlaneSampler {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

const SHADER: &str = "
@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    let pos = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
    return vec4f(pos[index], 0.0, 1.0);
}

@group(0) @binding(0) var y_tex: texture_2d<f32>;

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let y = textureLoad(y_tex, vec2i(pos.xy), 0).r;
    return vec4f(y, y, y, 1.0);
}
";

impl PlaneSampler {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
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

    /// One byte per luma texel, tightly packed.
    fn sample_luma(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &DecodedFrame,
    ) -> Vec<u8> {
        let size = wgpu::Extent3d {
            width: frame.width,
            height: frame.height,
            depth_or_array_layers: 1,
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&frame.y),
            }],
        });

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
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
            size,
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
        let mut luma = Vec::with_capacity((frame.width * frame.height) as usize);
        for row in 0..frame.height {
            let start = (row * bytes_per_row) as usize;
            luma.extend(
                mapped[start..start + (frame.width * 4) as usize]
                    .iter()
                    .step_by(4),
            );
        }
        luma
    }
}
