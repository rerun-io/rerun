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
    renderer::OutlineMaskProcessor,
    resource_managers::ResourceManagerError,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, RenderPipelineDesc,
        ShaderModuleDesc, TextureDesc,
    },
    ColorMap,
};

use super::{
    DrawData, DrawPhase, FileResolver, FileSystem, OutlineMaskPreference, RenderContext, Renderer,
    SharedRendererData, WgpuResourcePools,
};

// ---

mod gpu_data {
    // - Keep in sync with mirror in depth_cloud.wgsl.
    // - See `DepthCloud` for documentation.
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DepthCloudInfoUBO {
        pub depth_camera_extrinsics: crate::wgpu_buffer_types::Mat4,

        pub depth_camera_intrinsics: crate::wgpu_buffer_types::Mat3,

        /// Point radius is calculated as depth times this value.
        pub point_radius_from_normalized_depth: f32,

        pub colormap: u32,

        pub outline_mask_id: crate::wgpu_buffer_types::UVec2,

        pub end_padding: [crate::wgpu_buffer_types::PaddingRow; 16 - 8],
    }
}

/// The raw data from a depth texture.
///
/// This is either `u16` or `f32` values; in both cases the data will be uploaded to the shader
/// as-is.
/// For `u16`s, this results in a `Depth16Unorm` texture, otherwise an `R32Float`.
/// The reason we normalize `u16` is so that the shader can use a `float` texture in both cases.
/// However, it means we need to multiply the sampled value by `65535.0` in the shader to get
/// the actual depth.
///
/// The shader assumes that this is normalized, linear, non-flipped depth using the camera
/// position as reference point (not the camera plane!).
//
// TODO(cmc): support more depth data types.
// TODO(cmc): expose knobs to linearize/normalize/flip/cam-to-plane depth.
#[derive(Debug, Clone)]
pub enum DepthCloudDepthData {
    U16(crate::Buffer<u16>),
    F32(crate::Buffer<f32>),
}

impl Default for DepthCloudDepthData {
    fn default() -> Self {
        Self::F32(Default::default())
    }
}

pub struct DepthCloud {
    /// The extrinsics of the camera used for the projection.
    pub depth_camera_extrinsics: glam::Mat4,

    /// The intrinsics of the camera used for the projection.
    ///
    /// Only supports pinhole cameras at the moment.
    pub depth_camera_intrinsics: glam::Mat3,

    /// Point radius is calculated as depth times this value.
    pub point_radius_from_normalized_depth: f32,

    /// The dimensions of the depth texture in pixels.
    pub depth_dimensions: glam::UVec2,

    /// The actual data from the depth texture.
    ///
    /// See [`DepthCloudDepthData`] for more information.
    pub depth_data: DepthCloudDepthData,

    /// Configures color mapping mode.
    pub colormap: ColorMap,

    /// Option outline mask id preference.
    pub outline_mask_id: OutlineMaskPreference,
}

#[derive(Clone)]
struct DepthCloudDrawInstance {
    bind_group: GpuBindGroup,
    num_points: u32,
    render_outline_mask: bool,
}

#[derive(Clone)]
pub struct DepthCloudDrawData {
    instances: Vec<DepthCloudDrawInstance>,
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

        if depth_clouds.is_empty() {
            return Ok(DepthCloudDrawData {
                instances: Vec::new(),
            });
        }

        let depth_cloud_ubos = create_and_fill_uniform_buffer_batch(
            ctx,
            "depth_cloud_ubos".into(),
            depth_clouds.iter().map(|info| gpu_data::DepthCloudInfoUBO {
                depth_camera_extrinsics: info.depth_camera_extrinsics.into(),
                depth_camera_intrinsics: info.depth_camera_intrinsics.into(),
                point_radius_from_normalized_depth: info.point_radius_from_normalized_depth,
                colormap: info.colormap as u32,
                outline_mask_id: info.outline_mask_id.0.unwrap_or_default().into(),
                end_padding: Default::default(),
            }),
        );

        let mut instances = Vec::with_capacity(depth_clouds.len());
        for (depth_cloud, ubo) in depth_clouds.iter().zip(depth_cloud_ubos.into_iter()) {
            let depth_texture = match &depth_cloud.depth_data {
                DepthCloudDepthData::U16(data) => {
                    if cfg!(target_arch = "wasm32") {
                        // Web: manual normalization because Depth16Unorm textures aren't supported on
                        // the web (and won't ever be on the WebGL backend, see
                        // https://github.com/gfx-rs/wgpu/issues/3537).
                        //
                        // TODO(cmc): use an RG8 texture and unpack it manually in the shader instead.
                        use itertools::Itertools as _;
                        let dataf32 = data
                            .as_slice()
                            .iter()
                            .map(|d| *d as f32 / u16::MAX as f32)
                            .collect_vec();
                        create_and_upload_texture(
                            ctx,
                            depth_cloud,
                            dataf32.as_slice(),
                            wgpu::TextureFormat::R32Float,
                        )
                    } else {
                        // Native: We use Depth16Unorm over R16Unorm beacuse the latter is behind a feature flag,
                        // and not avilable on OpenGL backends.
                        create_and_upload_texture(
                            ctx,
                            depth_cloud,
                            data.as_slice(),
                            wgpu::TextureFormat::Depth16Unorm,
                        )
                    }
                }
                DepthCloudDepthData::F32(data) => create_and_upload_texture(
                    ctx,
                    depth_cloud,
                    data.as_slice(),
                    wgpu::TextureFormat::R32Float,
                ),
            };

            instances.push(DepthCloudDrawInstance {
                num_points: depth_cloud.depth_dimensions.x * depth_cloud.depth_dimensions.y,
                bind_group: ctx.gpu_resources.bind_groups.alloc(
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
                render_outline_mask: depth_cloud.outline_mask_id.is_some(),
            });
        }

        Ok(DepthCloudDrawData { instances })
    }
}

fn create_and_upload_texture<T: bytemuck::Pod>(
    ctx: &mut RenderContext,
    depth_cloud: &DepthCloud,
    data: &[T],
    depth_format: wgpu::TextureFormat,
) -> GpuTexture {
    crate::profile_function!();

    let depth_texture_size = wgpu::Extent3d {
        width: depth_cloud.depth_dimensions.x,
        height: depth_cloud.depth_dimensions.y,
        depth_or_array_layers: 1,
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
    let height_blocks = depth_cloud.depth_dimensions.y / format_info.block_dimensions.1 as u32;
    let bytes_per_row_unaligned = width_blocks * format_info.block_size as u32;

    // TODO(andreas): CpuGpuWriteBelt should make it easier to do this.
    let bytes_per_row_aligned =
        wgpu::util::align_to(bytes_per_row_unaligned, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let bytes_padding_per_row = (bytes_per_row_aligned - bytes_per_row_unaligned) as usize;
    // Sanity check the padding size. If this happens something is seriously wrong, as it would imply
    // that we can't express the required alignment with the block size.
    debug_assert!(
        bytes_padding_per_row % std::mem::size_of::<T>() == 0,
        "Padding is not a multiple of pixel size. Can't correctly pad the texture data"
    );
    let blocks_padding_per_row = bytes_padding_per_row / std::mem::size_of::<T>();

    let mut depth_texture_staging = ctx.cpu_write_gpu_read_belt.lock().allocate::<T>(
        &ctx.device,
        &ctx.gpu_resources.buffers,
        data.len() + blocks_padding_per_row * height_blocks as usize,
    );

    // Fill with a single copy if possible, otherwise do multiple, filling in padding.
    if blocks_padding_per_row == 0 {
        depth_texture_staging.extend_from_slice(data);
    } else {
        let row_padding = std::iter::repeat(T::zeroed())
            .take(blocks_padding_per_row)
            .collect::<Vec<_>>();

        for row in data.chunks(width_blocks as usize) {
            depth_texture_staging.extend_from_slice(row);
            depth_texture_staging.extend_from_slice(&row_padding);
        }
    }

    depth_texture_staging.copy_to_texture(
        ctx.active_frame.encoder.lock().get(),
        wgpu::ImageCopyTexture {
            texture: &depth_texture.inner.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        Some(NonZeroU32::new(bytes_per_row_aligned).expect("invalid bytes per row")),
        None,
        depth_texture_size,
    );

    depth_texture
}

pub struct DepthCloudRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for DepthCloudRenderer {
    type RendererDrawData = DepthCloudDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[DrawPhase::OutlineMask, DrawPhase::Opaque]
    }

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

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "DepthCloudRenderer::render_pipeline_desc_color".into(),
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
        };
        let render_pipeline_color = pools.render_pipelines.get_or_create(
            device,
            &render_pipeline_desc_color,
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        let render_pipeline_outline_mask = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "DepthCloudRenderer::render_pipeline_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                // Alpha to coverage doesn't work with the mask integer target.
                multisample: OutlineMaskProcessor::mask_default_msaa_state(
                    shared_data.config.hardware_tier,
                ),
                ..render_pipeline_desc_color
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        DepthCloudRenderer {
            render_pipeline_color,
            render_pipeline_outline_mask,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();
        if draw_data.instances.is_empty() {
            return Ok(());
        }

        let pipeline_handle = match phase {
            DrawPhase::OutlineMask => self.render_pipeline_outline_mask,
            DrawPhase::Opaque => self.render_pipeline_color,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let pipeline = pools.render_pipelines.get_resource(pipeline_handle)?;

        pass.set_pipeline(pipeline);

        for instance in &draw_data.instances {
            if phase == DrawPhase::OutlineMask && !instance.render_outline_mask {
                continue;
            }

            pass.set_bind_group(1, &instance.bind_group, &[]);
            pass.draw(0..instance.num_points * 6, 0..1);
        }

        Ok(())
    }
}
