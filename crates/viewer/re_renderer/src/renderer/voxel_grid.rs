use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::allocator::create_and_fill_uniform_buffer;
use crate::draw_phases::{DrawPhase, OutlineMaskProcessor};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupLayoutDesc, BufferDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuBuffer, GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, PipelineLayoutDesc,
    RenderPipelineDesc,
};
use crate::{
    Color32, CpuWriteGpuReadError, DepthOffset, DrawableCollector, OutlineMaskPreference,
    PickingLayerInstanceId, PickingLayerObjectId, PickingLayerProcessor, ViewBuilder,
    include_shader_module,
};

/// Number of vertices generated for each voxel cube.
const VERTICES_PER_VOXEL: u32 = 36;

/// A single sparse voxel to draw.
#[derive(Clone, Copy, Debug)]
pub struct VoxelGridInstance {
    /// Integer voxel index in local grid coordinates.
    pub index: glam::IVec3,

    /// Per-voxel sRGBA color.
    pub color: Color32,

    /// Picking-layer instance id for this voxel.
    pub picking_instance_id: PickingLayerInstanceId,
}

/// Batch-level options shared by all voxels in a draw-data object.
#[derive(Clone, Copy, Debug)]
pub struct VoxelGridOptions {
    /// Transform from local grid coordinates to world coordinates.
    pub world_from_grid: glam::Affine3A,

    /// Representative world-space position used for draw ordering.
    pub draw_order_position: glam::Vec3A,

    /// Uniform voxel side length in local grid units.
    pub cell_size: f32,

    /// Overall opacity multiplier.
    pub opacity: f32,

    /// Picking-layer object id shared by all voxels.
    pub picking_object_id: PickingLayerObjectId,

    /// Optional outline mask ids shared by all voxels.
    pub outline_mask_ids: OutlineMaskPreference,

    /// Depth offset used to resolve z-fighting.
    pub depth_offset: DepthOffset,
}

mod gpu_data {
    use crate::wgpu_buffer_types;
    use crate::{Color32, wgpu_resources::VertexBufferLayout};

    /// Keep in sync with `voxel_grid.wgsl`.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct InstanceData {
        pub index: [i32; 3],
        pub color: Color32,
        pub picking_instance_id: [u32; 2],
    }

    static_assertions::assert_eq_size!(InstanceData, [u8; 24]);

    impl InstanceData {
        pub fn vertex_buffer_layout() -> VertexBufferLayout {
            VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as _,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: VertexBufferLayout::attributes_from_formats(
                    0,
                    [
                        wgpu::VertexFormat::Sint32x3,
                        wgpu::VertexFormat::Unorm8x4,
                        wgpu::VertexFormat::Uint32x2,
                    ]
                    .into_iter(),
                ),
            }
        }
    }

    /// Keep in sync with `voxel_grid.wgsl`.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub world_from_grid: wgpu_buffer_types::Mat4,
        pub cell_size_opacity_depth_offset: wgpu_buffer_types::Vec4,
        pub picking_object_id: wgpu_buffer_types::UVec2,
        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 10],
    }
}

#[derive(Clone)]
pub struct VoxelGridDrawData {
    instance_buffer: Option<GpuBuffer>,
    bind_group: GpuBindGroup,
    voxel_count: u32,
    draw_phase: DrawPhase,
    draw_outline_mask: bool,
    position: glam::Vec3A,
}

impl DrawData for VoxelGridDrawData {
    type Renderer = VoxelGridRenderer;

    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        if self.voxel_count == 0 {
            return;
        }

        let drawable = DrawDataDrawable::from_world_position(view_info, self.position, 0);
        collector.add_drawable(self.draw_phase, drawable);
        collector.add_drawable(DrawPhase::PickingLayer, drawable);
        if self.draw_outline_mask {
            collector.add_drawable(DrawPhase::OutlineMask, drawable);
        }
    }
}

impl VoxelGridDrawData {
    /// Creates compact GPU draw data for a sparse voxel grid.
    pub fn new(
        ctx: &RenderContext,
        instances: &[VoxelGridInstance],
        options: VoxelGridOptions,
    ) -> Result<Self, CpuWriteGpuReadError> {
        re_tracing::profile_function!();

        let renderer = ctx.renderer::<VoxelGridRenderer>();
        let opacity = options.opacity.clamp(0.0, 1.0);
        let voxel_count = if opacity == 0.0 {
            0
        } else {
            instances.len() as u32
        };
        let draw_phase =
            if opacity < 1.0 || instances.iter().any(|instance| instance.color.a() < 255) {
                DrawPhase::Transparent
            } else {
                DrawPhase::Opaque
            };

        let instance_buffer = if voxel_count == 0 {
            None
        } else {
            let instance_buffer = ctx.gpu_resources.buffers.alloc(
                &ctx.device,
                &BufferDesc {
                    label: "VoxelGridDrawData::instance_buffer".into(),
                    size: (std::mem::size_of::<gpu_data::InstanceData>() * instances.len()) as _,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            );

            let mut staging = ctx
                .cpu_write_gpu_read_belt
                .lock()
                .allocate::<gpu_data::InstanceData>(
                    &ctx.device,
                    &ctx.gpu_resources.buffers,
                    instances.len(),
                )?;

            staging.extend(instances.iter().map(|instance| gpu_data::InstanceData {
                index: instance.index.to_array(),
                color: instance.color,
                picking_instance_id: [
                    instance.picking_instance_id.0 as u32,
                    (instance.picking_instance_id.0 >> 32) as u32,
                ],
            }))?;
            staging.copy_to_buffer(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                &instance_buffer,
                0,
            )?;

            Some(instance_buffer)
        };

        let uniform_buffer_binding = create_and_fill_uniform_buffer(
            ctx,
            "VoxelGridDrawData::uniform_buffer".into(),
            gpu_data::UniformBuffer {
                world_from_grid: options.world_from_grid.into(),
                cell_size_opacity_depth_offset: glam::Vec4::new(
                    options.cell_size,
                    opacity,
                    options.depth_offset as f32,
                    0.0,
                )
                .into(),
                picking_object_id: glam::UVec2::new(
                    options.picking_object_id.0 as u32,
                    (options.picking_object_id.0 >> 32) as u32,
                )
                .into(),
                outline_mask_ids: options
                    .outline_mask_ids
                    .0
                    .map_or([0, 0], |mask| mask)
                    .into(),
                end_padding: Default::default(),
            },
        );

        Ok(Self {
            instance_buffer,
            bind_group: ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: "VoxelGridDrawData::bind_group".into(),
                    entries: smallvec![uniform_buffer_binding],
                    layout: renderer.bind_group_layout,
                },
            ),
            voxel_count,
            draw_phase,
            draw_outline_mask: options.outline_mask_ids.is_some(),
            position: options.draw_order_position,
        })
    }

    #[inline]
    pub const fn gpu_instance_size_bytes() -> usize {
        std::mem::size_of::<gpu_data::InstanceData>()
    }

    #[inline]
    pub fn voxel_count(&self) -> u32 {
        self.voxel_count
    }
}

pub struct VoxelGridRenderer {
    rp_opaque: GpuRenderPipelineHandle,
    rp_transparent: GpuRenderPipelineHandle,
    rp_picking_layer: GpuRenderPipelineHandle,
    rp_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for VoxelGridRenderer {
    type RendererDrawData = VoxelGridDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "VoxelGridRenderer::bind_group_layout".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: (std::mem::size_of::<gpu_data::UniformBuffer>() as u64)
                            .try_into()
                            .ok(),
                    },
                    count: None,
                }],
            },
        );
        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "VoxelGridRenderer::pipeline_layout".into(),
                entries: vec![ctx.global_bindings.layout, bind_group_layout],
            },
        );

        let shader_module = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &include_shader_module!("../../shader/voxel_grid.wgsl"));

        let vertex_buffers = smallvec![gpu_data::InstanceData::vertex_buffer_layout()];
        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        };
        let rp_opaque_desc = RenderPipelineDesc {
            label: "VoxelGridRenderer::rp_opaque".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers,
            render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
            primitive,
            depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE),
            multisample: ViewBuilder::main_target_default_msaa_state(ctx.render_config(), false),
        };

        let rp_opaque = ctx
            .gpu_resources
            .render_pipelines
            .get_or_create(ctx, &rp_opaque_desc);
        let rp_transparent = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "VoxelGridRenderer::rp_transparent".into(),
                render_targets: smallvec![Some(wgpu::ColorTargetState {
                    format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE_NO_WRITE),
                ..rp_opaque_desc.clone()
            },
        );
        let rp_picking_layer = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "VoxelGridRenderer::rp_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..rp_opaque_desc.clone()
            },
        );
        let rp_outline_mask = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "VoxelGridRenderer::rp_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
                ..rp_opaque_desc
            },
        );

        Self {
            rp_opaque,
            rp_transparent,
            rp_picking_layer,
            rp_outline_mask,
            bind_group_layout,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        re_tracing::profile_function!();

        let pipeline = match phase {
            DrawPhase::Opaque => self.rp_opaque,
            DrawPhase::Transparent => self.rp_transparent,
            DrawPhase::PickingLayer => self.rp_picking_layer,
            DrawPhase::OutlineMask => self.rp_outline_mask,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        pass.set_pipeline(render_pipelines.get(pipeline)?);

        for DrawInstruction {
            draw_data,
            drawables,
        } in draw_instructions
        {
            let Some(instance_buffer) = &draw_data.instance_buffer else {
                continue;
            };
            pass.set_bind_group(1, &draw_data.bind_group, &[]);
            pass.set_vertex_buffer(0, instance_buffer.slice(..));

            for _drawable in *drawables {
                pass.draw(0..VERTICES_PER_VOXEL, 0..draw_data.voxel_count);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use enumset::EnumSet;

    use super::*;
    use crate::renderer::DrawableCollectionViewInfo;
    use crate::{DrawPhaseManager, PickingLayerObjectId};

    #[test]
    fn test_phase_collection() {
        let ctx = RenderContext::new_test();
        let draw_data = VoxelGridDrawData::new(
            &ctx,
            &[
                VoxelGridInstance {
                    index: glam::IVec3::new(-1, 0, 2),
                    color: Color32::RED,
                    picking_instance_id: PickingLayerInstanceId(7),
                },
                VoxelGridInstance {
                    index: glam::IVec3::ONE,
                    color: Color32::GREEN,
                    picking_instance_id: PickingLayerInstanceId(8),
                },
            ],
            VoxelGridOptions {
                world_from_grid: glam::Affine3A::IDENTITY,
                draw_order_position: glam::Vec3A::ZERO,
                cell_size: 0.25,
                opacity: 1.0,
                picking_object_id: PickingLayerObjectId(42),
                outline_mask_ids: OutlineMaskPreference::some(1, 2),
                depth_offset: 0,
            },
        )
        .unwrap();

        assert_eq!(draw_data.voxel_count(), 2);

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(
            &ctx,
            draw_data.into(),
            &DrawableCollectionViewInfo {
                camera_world_position: glam::Vec3A::ZERO,
            },
        );

        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::Opaque)
                .len(),
            1
        );
        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::PickingLayer)
                .len(),
            1
        );
        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::OutlineMask)
                .len(),
            1
        );
        assert!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::Transparent)
                .is_empty()
        );
    }

    #[test]
    fn zero_opacity_collects_no_drawables() {
        let ctx = RenderContext::new_test();
        let draw_data = VoxelGridDrawData::new(
            &ctx,
            &[VoxelGridInstance {
                index: glam::IVec3::ZERO,
                color: Color32::RED,
                picking_instance_id: PickingLayerInstanceId(7),
            }],
            VoxelGridOptions {
                world_from_grid: glam::Affine3A::IDENTITY,
                draw_order_position: glam::Vec3A::ZERO,
                cell_size: 0.25,
                opacity: 0.0,
                picking_object_id: PickingLayerObjectId(42),
                outline_mask_ids: OutlineMaskPreference::some(1, 2),
                depth_offset: 0,
            },
        )
        .unwrap();

        assert_eq!(draw_data.voxel_count(), 0);

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(
            &ctx,
            draw_data.into(),
            &DrawableCollectionViewInfo {
                camera_world_position: glam::Vec3A::ZERO,
            },
        );

        for phase in [
            DrawPhase::Opaque,
            DrawPhase::PickingLayer,
            DrawPhase::OutlineMask,
            DrawPhase::Transparent,
        ] {
            assert!(draw_phase_manager.drawables_for_phase(phase).is_empty());
        }
    }

    #[test]
    fn transparent_opacity_collects_transparent_drawables() {
        let ctx = RenderContext::new_test();
        let draw_data = VoxelGridDrawData::new(
            &ctx,
            &[VoxelGridInstance {
                index: glam::IVec3::ZERO,
                color: Color32::RED,
                picking_instance_id: PickingLayerInstanceId(7),
            }],
            VoxelGridOptions {
                world_from_grid: glam::Affine3A::IDENTITY,
                draw_order_position: glam::Vec3A::ZERO,
                cell_size: 0.25,
                opacity: 0.5,
                picking_object_id: PickingLayerObjectId(42),
                outline_mask_ids: OutlineMaskPreference::some(1, 2),
                depth_offset: 0,
            },
        )
        .unwrap();

        assert_eq!(draw_data.voxel_count(), 1);

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(
            &ctx,
            draw_data.into(),
            &DrawableCollectionViewInfo {
                camera_world_position: glam::Vec3A::ZERO,
            },
        );

        assert!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::Opaque)
                .is_empty()
        );
        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::Transparent)
                .len(),
            1
        );
        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::PickingLayer)
                .len(),
            1
        );
        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::OutlineMask)
                .len(),
            1
        );
    }
}
