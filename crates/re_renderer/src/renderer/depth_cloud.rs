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

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    draw_phases::{DrawPhase, OutlineMaskProcessor},
    include_shader_module,
    resource_managers::{GpuTexture2D, ResourceManagerError},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
    },
    Colormap, OutlineMaskPreference, PickingLayerObjectId, PickingLayerProcessor,
};

use super::{
    DrawData, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

// ---

mod gpu_data {
    use crate::{wgpu_buffer_types, PickingLayerObjectId};

    /// Keep in sync with mirror in `depth_cloud.wgsl.`
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DepthCloudInfoUBO {
        /// The extrinsics of the camera used for the projection.
        pub world_from_obj: wgpu_buffer_types::Mat4,

        pub depth_camera_intrinsics: wgpu_buffer_types::Mat3,

        pub outline_mask_id: wgpu_buffer_types::UVec2,
        pub picking_layer_object_id: PickingLayerObjectId,

        /// Multiplier to get world-space depth from whatever is in the texture.
        pub world_depth_from_texture_value: f32,

        /// Point radius is calculated as world-space depth times this value.
        pub point_radius_from_world_depth: f32,

        /// The maximum depth value in world-space, for use with the colormap.
        pub max_depth_in_world: f32,

        /// Which colormap should be used.
        pub colormap: u32,

        /// Changes over different draw-phases.
        pub radius_boost_in_ui_points: wgpu_buffer_types::F32RowPadded,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 4 - 3 - 1 - 1 - 1],
    }

    impl DepthCloudInfoUBO {
        pub fn from_depth_cloud(
            radius_boost_in_ui_points: f32,
            depth_cloud: &super::DepthCloud,
        ) -> Self {
            let super::DepthCloud {
                world_from_obj,
                depth_camera_intrinsics,
                world_depth_from_texture_depth,
                point_radius_from_world_depth,
                max_depth_in_world,
                depth_dimensions: _,
                depth_texture: _,
                colormap,
                outline_mask_id,
                picking_object_id,
            } = depth_cloud;

            Self {
                world_from_obj: (*world_from_obj).into(),
                depth_camera_intrinsics: (*depth_camera_intrinsics).into(),
                outline_mask_id: outline_mask_id.0.unwrap_or_default().into(),
                world_depth_from_texture_value: *world_depth_from_texture_depth,
                point_radius_from_world_depth: *point_radius_from_world_depth,
                max_depth_in_world: *max_depth_in_world,
                colormap: *colormap as u32,
                radius_boost_in_ui_points: radius_boost_in_ui_points.into(),
                picking_layer_object_id: *picking_object_id,
                end_padding: Default::default(),
            }
        }
    }
}

pub struct DepthCloud {
    /// The extrinsics of the camera used for the projection.
    pub world_from_obj: glam::Mat4,

    /// The intrinsics of the camera used for the projection.
    ///
    /// Only supports pinhole cameras at the moment.
    pub depth_camera_intrinsics: glam::Mat3,

    /// Multiplier to get world-space depth from whatever is in [`Self::depth_texture`].
    pub world_depth_from_texture_depth: f32,

    /// Point radius is calculated as world-space depth times this value.
    pub point_radius_from_world_depth: f32,

    /// The maximum depth value in world-space, for use with the colormap.
    pub max_depth_in_world: f32,

    /// The dimensions of the depth texture in pixels.
    pub depth_dimensions: glam::UVec2,

    /// The actual data from the depth texture.
    ///
    /// Only F32 data is supported right now.
    pub depth_texture: GpuTexture2D,

    /// Configures color mapping mode.
    pub colormap: Colormap,

    /// Option outline mask id preference.
    pub outline_mask_id: OutlineMaskPreference,

    /// Picking object id that applies for the entire depth cloud.
    pub picking_object_id: PickingLayerObjectId,
}

impl DepthCloud {
    /// World-space bounding-box.
    pub fn bbox(&self) -> macaw::BoundingBox {
        let max_depth = self.max_depth_in_world;
        let w = self.depth_dimensions.x as f32;
        let h = self.depth_dimensions.y as f32;
        let corners = [
            glam::Vec3::ZERO, // camera origin
            glam::Vec3::new(0.0, 0.0, max_depth),
            glam::Vec3::new(0.0, h, max_depth),
            glam::Vec3::new(w, 0.0, max_depth),
            glam::Vec3::new(w, h, max_depth),
        ];

        let intrinsics = self.depth_camera_intrinsics;
        let focal_length = glam::vec2(intrinsics.col(0).x, intrinsics.col(1).y);
        let offset = intrinsics.col(2).truncate();

        let mut bbox = macaw::BoundingBox::nothing();

        for corner in corners {
            let depth = corner.z;
            let pos_in_obj = ((corner.truncate() - offset) * depth / focal_length).extend(depth);
            let pos_in_world = self.world_from_obj.project_point3(pos_in_obj);
            bbox.extend(pos_in_world);
        }

        bbox
    }
}

pub struct DepthClouds {
    pub clouds: Vec<DepthCloud>,
    pub radius_boost_in_ui_points_for_outlines: f32,
}

#[derive(Clone)]
struct DepthCloudDrawInstance {
    bind_group_opaque: GpuBindGroup,
    bind_group_outline: GpuBindGroup,
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
        depth_clouds: &DepthClouds,
    ) -> Result<Self, ResourceManagerError> {
        crate::profile_function!();

        let DepthClouds {
            clouds: depth_clouds,
            radius_boost_in_ui_points_for_outlines,
        } = depth_clouds;

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

        let depth_cloud_ubo_binding_outlines = create_and_fill_uniform_buffer_batch(
            ctx,
            "depth_cloud_ubos".into(),
            depth_clouds.iter().map(|dc| {
                gpu_data::DepthCloudInfoUBO::from_depth_cloud(
                    *radius_boost_in_ui_points_for_outlines,
                    dc,
                )
            }),
        );
        let depth_cloud_ubo_binding_opaque = create_and_fill_uniform_buffer_batch(
            ctx,
            "depth_cloud_ubos".into(),
            depth_clouds
                .iter()
                .map(|dc| gpu_data::DepthCloudInfoUBO::from_depth_cloud(0.0, dc)),
        );

        let mut instances = Vec::with_capacity(depth_clouds.len());
        for (depth_cloud, ubo_outlines, ubo_opaque) in itertools::izip!(
            depth_clouds,
            depth_cloud_ubo_binding_outlines,
            depth_cloud_ubo_binding_opaque
        ) {
            let mk_bind_group = |label, ubo: BindGroupEntry| {
                ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &ctx.gpu_resources,
                    &BindGroupDesc {
                        label,
                        entries: smallvec![
                            ubo,
                            BindGroupEntry::DefaultTextureView(depth_cloud.depth_texture.handle),
                        ],
                        layout: bg_layout,
                    },
                )
            };

            let bind_group_outline = mk_bind_group("depth_cloud_outline".into(), ubo_outlines);
            let bind_group_opaque = mk_bind_group("depth_cloud_opaque".into(), ubo_opaque);

            instances.push(DepthCloudDrawInstance {
                num_points: depth_cloud.depth_dimensions.x * depth_cloud.depth_dimensions.y,
                bind_group_opaque,
                bind_group_outline,
                render_outline_mask: depth_cloud.outline_mask_id.is_some(),
            });
        }

        Ok(DepthCloudDrawData { instances })
    }
}

pub struct DepthCloudRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for DepthCloudRenderer {
    type RendererDrawData = DepthCloudDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[
            DrawPhase::Opaque,
            DrawPhase::PickingLayer,
            DrawPhase::OutlineMask,
        ]
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
            &include_shader_module!("../../shader/depth_cloud.wgsl"),
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
        let render_pipeline_picking_layer = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "DepthCloudRenderer::render_pipeline_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color.clone()
            },
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
            render_pipeline_picking_layer,
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
            DrawPhase::Opaque => self.render_pipeline_color,
            DrawPhase::PickingLayer => self.render_pipeline_picking_layer,
            DrawPhase::OutlineMask => self.render_pipeline_outline_mask,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let pipeline = pools.render_pipelines.get_resource(pipeline_handle)?;

        pass.set_pipeline(pipeline);

        for instance in &draw_data.instances {
            if phase == DrawPhase::OutlineMask && !instance.render_outline_mask {
                continue;
            }

            let bind_group = match phase {
                DrawPhase::OutlineMask => &instance.bind_group_outline,
                DrawPhase::PickingLayer | DrawPhase::Opaque => &instance.bind_group_opaque,
                _ => unreachable!(),
            };

            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(0..instance.num_points * 6, 0..1);
        }

        Ok(())
    }
}
