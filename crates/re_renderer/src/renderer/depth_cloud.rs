//! Renderer that makes it easy to draw point clouds straight out of depth textures.
//!
//! Textures are uploaded just-in-time, no caching.
//!
//! ## Implementation details
//!
//! Since there's no widespread support for bindless textures, this requires one bind group and one
//! draw call per texture.
//! This is a pretty heavy shader though, so the overhead is minimal.
//!
//! The vertex shader backprojects the depth texture using the user-specified intrinsics, and then
//! behaves pretty much exactly like our point cloud renderer (see [`point_cloud.rs`]).

use smallvec::smallvec;
use std::num::NonZeroU32;

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    include_file,
    resource_managers::ResourceManagerError,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, RenderPipelineDesc,
        ShaderModuleDesc, TextureDesc,
    },
};

use super::{
    DrawData, DrawPhase, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

// ---

mod gpu_data {
    // - Keep in sync with mirror in depth_cloud.wgsl.
    // - See `DepthCloud` for documentation.
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DepthCloudInfoUBO {
        pub world_from_obj: crate::wgpu_buffer_types::Mat4,
        pub depth_camera_intrinsics: crate::wgpu_buffer_types::Mat3,
        pub radius_scale: crate::wgpu_buffer_types::F32RowPadded,

        pub end_padding: [crate::wgpu_buffer_types::PaddingRow; 16 - 8],
    }
}

/// The raw data from a depth texture.
///
/// This is either `u16` or `f32` values; in both cases the data will be uploaded to the shader
/// as-is.
/// For `u16`s, this results in a `Depth16Unorm` texture, otherwise an `R32Float`.
///
/// The shader assumes that this is normalized, linear, non-flipped depth using the camera
/// position as reference point (not the camera plane!).
//
// TODO(cmc): support more depth data types.
// TODO(cmc): expose knobs to linearize/normalize/flip/cam-to-plane depth.
#[derive(Debug, Clone)]
pub enum DepthCloudDepthData {
    U16(Vec<u16>),
    F32(Vec<f32>),
}

pub struct DepthCloud {
    pub world_from_obj: glam::Mat4,

    /// The intrinsics of the camera used for the projection.
    ///
    /// Only supports pinhole cameras at the moment.
    pub depth_camera_intrinsics: glam::Mat3,

    /// The scale to apply to the radii of the backprojected points.
    pub radius_scale: f32,

    /// The dimensions of the depth texture in pixels.
    pub depth_dimensions: glam::UVec2,

    /// The actual data from the depth texture.
    ///
    /// See [`DepthCloudDepthData`] for more information.
    pub depth_data: DepthCloudDepthData,
}

impl Default for DepthCloud {
    fn default() -> Self {
        Self {
            world_from_obj: glam::Mat4::IDENTITY,
            depth_camera_intrinsics: glam::Mat3::IDENTITY,
            radius_scale: 1.0,
            depth_dimensions: glam::UVec2::ZERO,
            depth_data: DepthCloudDepthData::F32(Vec::new()),
        }
    }
}

#[derive(Clone)]
pub struct DepthCloudDrawData {
    // Every single point clouds and their respective total number of points.
    bind_groups: Vec<(u32, GpuBindGroup)>,
}

impl DrawData for DepthCloudDrawData {
    type Renderer = DepthCloudRenderer;
}

impl DepthCloudDrawData {
    pub fn new(
        ctx: &mut RenderContext,
        depth_clouds: &[DepthCloud],
    ) -> Result<Self, ResourceManagerError> {
        crate::profile_function!();

        if depth_clouds.is_empty() {
            return Ok(DepthCloudDrawData {
                bind_groups: Vec::new(),
            });
        }

        let depth_cloud_ubos = create_and_fill_uniform_buffer_batch(
            ctx,
            "depth_cloud_ubos".into(),
            depth_clouds.iter().map(|info| gpu_data::DepthCloudInfoUBO {
                world_from_obj: info.world_from_obj.into(),
                depth_camera_intrinsics: info.depth_camera_intrinsics.into(),
                radius_scale: info.radius_scale.into(),
                end_padding: Default::default(),
            }),
        );

        let bg_layout = ctx
            .renderers
            .write()
            .get_or_create::<_, DepthCloudRenderer>(
                &ctx.shared_renderer_data,
                &mut ctx.gpu_resources,
                &ctx.device,
                &mut ctx.resolver,
            )
            .bind_group_layout;

        let mut bind_groups = Vec::with_capacity(depth_clouds.len());
        for (depth_cloud, ubo) in depth_clouds.iter().zip(depth_cloud_ubos.into_iter()) {
            let depth_texture = match &depth_cloud.depth_data {
                // On native, we can use D16 textures without issues.
                #[cfg(not(target_arch = "wasm32"))]
                DepthCloudDepthData::U16(data) => {
                    create_and_upload_texture(ctx, depth_cloud, data.as_slice(), false)
                }
                // On the web, OTOH, they are currently broken, see
                // https://github.com/gfx-rs/wgpu/issues/3537.
                #[cfg(target_arch = "wasm32")]
                DepthCloudDepthData::U16(data) => {
                    let dataf32 = data
                        .as_slice()
                        .iter()
                        .map(|d| *d as f32 / u16::MAX as f32)
                        .collect_vec();
                    create_and_upload_texture(ctx, depth_cloud, dataf32.as_slice(), true)
                }
                DepthCloudDepthData::F32(data) => {
                    create_and_upload_texture(ctx, depth_cloud, data.as_slice(), false)
                }
            };

            bind_groups.push((
                depth_cloud.depth_dimensions.x * depth_cloud.depth_dimensions.y,
                ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &ctx.gpu_resources,
                    &BindGroupDesc {
                        label: "depth_cloud_bg".into(),
                        entries: smallvec![
                            ubo,
                            BindGroupEntry::DefaultTextureView(depth_texture.handle),
                        ],
                        layout: bg_layout,
                    },
                ),
            ));
        }

        Ok(DepthCloudDrawData { bind_groups })
    }
}

fn create_and_upload_texture<T: bytemuck::Pod>(
    ctx: &mut RenderContext,
    depth_cloud: &DepthCloud,
    data: &[T],
    force_32bit: bool,
) -> GpuTexture {
    crate::profile_function!();

    let depth_texture_size = wgpu::Extent3d {
        width: depth_cloud.depth_dimensions.x,
        height: depth_cloud.depth_dimensions.y,
        depth_or_array_layers: 1,
    };
    let depth_format = if force_32bit {
        wgpu::TextureFormat::R32Float
    } else {
        match depth_cloud.depth_data {
            DepthCloudDepthData::U16(_) => wgpu::TextureFormat::Depth16Unorm,
            DepthCloudDepthData::F32(_) => wgpu::TextureFormat::R32Float,
        }
    };
    let depth_texture_desc = TextureDesc {
        label: "depth_texture".into(),
        size: depth_texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: depth_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    };
    let depth_texture = ctx
        .gpu_resources
        .textures
        .alloc(&ctx.device, &depth_texture_desc);

    let format_info = depth_texture_desc.format.describe();
    let width_blocks = depth_cloud.depth_dimensions.x / format_info.block_dimensions.0 as u32;
    let bytes_per_row_unaligned = width_blocks * format_info.block_size as u32;

    let mut depth_texture_staging = ctx.cpu_write_gpu_read_belt.lock().allocate::<T>(
        &ctx.device,
        &ctx.gpu_resources.buffers,
        data.len(),
    );
    depth_texture_staging.extend_from_slice(data);

    depth_texture_staging.copy_to_texture(
        ctx.active_frame.encoder.lock().get(),
        wgpu::ImageCopyTexture {
            texture: &depth_texture.inner.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        Some(NonZeroU32::new(bytes_per_row_unaligned).expect("invalid bytes per row")),
        None,
        depth_texture_size,
    );

    depth_texture
}

pub struct DepthCloudRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for DepthCloudRenderer {
    type RendererDrawData = DepthCloudDrawData;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        crate::profile_function!();

        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "depth_cloud_bg_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<gpu_data::DepthCloudInfoUBO>()
                                as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "depth_cloud_rp_layout".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "depth_cloud".into(),
                source: include_file!("../../shader/depth_cloud.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "depth_cloud_rp".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
                multisample: wgpu::MultisampleState {
                    // We discard pixels to do the round cutout, therefore we need to
                    // calculate our own sampling mask.
                    alpha_to_coverage_enabled: true,
                    ..ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE
                },
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        DepthCloudRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        _phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();
        if draw_data.bind_groups.is_empty() {
            return Ok(());
        }

        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        pass.set_pipeline(pipeline);

        for (num_points, bind_group) in &draw_data.bind_groups {
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(0..*num_points * 6, 0..1);
        }

        Ok(())
    }
}
