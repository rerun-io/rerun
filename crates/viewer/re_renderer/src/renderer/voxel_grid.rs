use std::ops::Range;

use enumset::{EnumSet, enum_set};
use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::allocator::{create_and_fill_uniform_buffer, create_and_fill_uniform_buffer_batch};
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
#[derive(Clone, Debug)]
pub struct VoxelGridOptions {
    /// Transform from local grid coordinates to world coordinates.
    pub world_from_grid: glam::Affine3A,

    /// Representative world-space position used for draw ordering.
    pub draw_order_position: glam::Vec3A,

    /// Voxel dimensions in local grid units.
    pub voxel_size: glam::Vec3,

    /// Picking-layer object id shared by all voxels.
    pub picking_object_id: PickingLayerObjectId,

    /// Optional outline mask ids shared by all voxels.
    pub outline_mask_ids: OutlineMaskPreference,

    /// Outline masks for individual voxel instance ranges.
    ///
    /// Ranges are relative to the `instances` slice passed to [`VoxelGridDrawData::new`].
    ///
    /// Each range is drawn as an extra draw call in the outline-mask phase only, so this is
    /// meant for a limited number of "extra selections" (e.g. picked voxels), not bulk highlighting.
    /// These override the overall mask for the covered voxels.
    pub additional_outline_mask_ids_instance_ranges: Vec<(Range<u32>, OutlineMaskPreference)>,

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
        pub voxel_size_depth_offset: wgpu_buffer_types::Vec4,
        pub picking_object_id: wgpu_buffer_types::UVec2,
        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 10],
    }
}

/// A single draw call covering a range of voxel instances.
#[derive(Clone)]
struct VoxelGridDraw {
    bind_group: GpuBindGroup,
    instance_range: Range<u32>,
    active_phases: EnumSet<DrawPhase>,
}

#[derive(Clone)]
pub struct VoxelGridDrawData {
    instance_buffer: Option<GpuBuffer>,
    voxel_count: u32,

    /// Outline draws can draw only a small subset of the instances (==cubes) covered by the overall buffer.
    /// Typically, the first element in this list is the entire range.
    draws: Vec<VoxelGridDraw>,

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

        for (draw_idx, draw) in self.draws.iter().enumerate() {
            collector.add_drawable(
                draw.active_phases,
                DrawDataDrawable::from_world_position(view_info, self.position, draw_idx as _),
            );
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

        let VoxelGridOptions {
            world_from_grid,
            draw_order_position,
            voxel_size,
            picking_object_id,
            outline_mask_ids,
            additional_outline_mask_ids_instance_ranges,
            depth_offset,
        } = options;

        let renderer = ctx.renderer::<VoxelGridRenderer>();
        let voxel_count = instances.len() as u32;
        let draw_phase = if instances.iter().any(|instance| instance.color.a() < 255) {
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

        let picking_object_id_packed = glam::UVec2::new(
            picking_object_id.0 as u32,
            (picking_object_id.0 >> 32) as u32,
        );
        let make_uniform = |mask: OutlineMaskPreference| gpu_data::UniformBuffer {
            world_from_grid: world_from_grid.into(),
            voxel_size_depth_offset: voxel_size.extend(depth_offset as f32).into(),
            picking_object_id: picking_object_id_packed.into(),
            outline_mask_ids: mask.0.map_or([0, 0], |mask| mask).into(),
            end_padding: Default::default(),
        };

        let make_bind_group = |label: &str, uniform_binding| {
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: label.into(),
                    entries: smallvec![uniform_binding],
                    layout: renderer.bind_group_layout,
                },
            )
        };

        // First draw covers all voxels for color, picking, and the overall outline mask (if any).
        let mut overall_phases = enum_set![draw_phase | DrawPhase::PickingLayer];
        if outline_mask_ids.is_some() {
            overall_phases.insert(DrawPhase::OutlineMask);
        }
        let mut draws = vec![VoxelGridDraw {
            bind_group: make_bind_group(
                "VoxelGridDrawData::bind_group",
                create_and_fill_uniform_buffer(
                    ctx,
                    "VoxelGridDrawData::uniform_buffer".into(),
                    make_uniform(outline_mask_ids),
                ),
            ),
            instance_range: 0..voxel_count,
            active_phases: overall_phases,
        }];

        // Generate an extra outline-mask-only draw for each individually highlighted voxel range.
        // Costly if there are many (each needs its own uniform buffer & draw call), but cheap if
        // there are only a few, which is the expected case (e.g. a single picked voxel).
        if !additional_outline_mask_ids_instance_ranges.is_empty() {
            let sub_draw_uniform_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "VoxelGridDrawData::outline_mask_uniform_buffers".into(),
                additional_outline_mask_ids_instance_ranges
                    .iter()
                    .map(|(_, mask)| make_uniform(*mask)),
            );

            draws.extend(
                std::iter::zip(
                    additional_outline_mask_ids_instance_ranges,
                    sub_draw_uniform_bindings,
                )
                .map(|((range, _), uniform_binding)| VoxelGridDraw {
                    bind_group: make_bind_group(
                        "VoxelGridDrawData::outline_mask_bind_group",
                        uniform_binding,
                    ),
                    instance_range: range,
                    active_phases: enum_set![DrawPhase::OutlineMask],
                }),
            );
        }

        Ok(Self {
            instance_buffer,
            voxel_count,
            draws,
            position: draw_order_position,
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
            pass.set_vertex_buffer(0, instance_buffer.slice(..));

            for drawable in *drawables {
                let draw = &draw_data.draws[drawable.draw_data_payload as usize];
                pass.set_bind_group(1, &draw.bind_group, &[]);
                pass.draw(0..VERTICES_PER_VOXEL, draw.instance_range.clone());
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
                voxel_size: glam::Vec3::splat(0.25),
                picking_object_id: PickingLayerObjectId(42),
                outline_mask_ids: OutlineMaskPreference::some(1, 2),
                additional_outline_mask_ids_instance_ranges: Vec::new(),
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
    fn transparent_color_collects_transparent_drawables() {
        let ctx = RenderContext::new_test();
        let draw_data = VoxelGridDrawData::new(
            &ctx,
            &[VoxelGridInstance {
                index: glam::IVec3::ZERO,
                #[expect(clippy::disallowed_methods)]
                color: Color32::from_rgba_unmultiplied(255, 0, 0, 128),
                picking_instance_id: PickingLayerInstanceId(7),
            }],
            VoxelGridOptions {
                world_from_grid: glam::Affine3A::IDENTITY,
                draw_order_position: glam::Vec3A::ZERO,
                voxel_size: glam::Vec3::splat(0.25),
                picking_object_id: PickingLayerObjectId(42),
                outline_mask_ids: OutlineMaskPreference::some(1, 2),
                additional_outline_mask_ids_instance_ranges: Vec::new(),
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

    #[test]
    fn per_instance_outline_masks_add_extra_outline_drawables() {
        let ctx = RenderContext::new_test();
        let draw_data = VoxelGridDrawData::new(
            &ctx,
            &[
                VoxelGridInstance {
                    index: glam::IVec3::ZERO,
                    color: Color32::RED,
                    picking_instance_id: PickingLayerInstanceId(0),
                },
                VoxelGridInstance {
                    index: glam::IVec3::ONE,
                    color: Color32::GREEN,
                    picking_instance_id: PickingLayerInstanceId(1),
                },
                VoxelGridInstance {
                    index: glam::IVec3::splat(2),
                    color: Color32::BLUE,
                    picking_instance_id: PickingLayerInstanceId(2),
                },
            ],
            VoxelGridOptions {
                world_from_grid: glam::Affine3A::IDENTITY,
                draw_order_position: glam::Vec3A::ZERO,
                voxel_size: glam::Vec3::splat(0.25),
                picking_object_id: PickingLayerObjectId(42),
                // No overall outline, but two individually highlighted voxels.
                outline_mask_ids: OutlineMaskPreference::NONE,
                additional_outline_mask_ids_instance_ranges: vec![
                    (0..1, OutlineMaskPreference::some(1, 0)),
                    (2..3, OutlineMaskPreference::some(2, 0)),
                ],
                depth_offset: 0,
            },
        )
        .unwrap();

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(
            &ctx,
            draw_data.into(),
            &DrawableCollectionViewInfo {
                camera_world_position: glam::Vec3A::ZERO,
            },
        );

        // No overall mask, so the only outline drawables are the two per-instance ones.
        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::OutlineMask)
                .len(),
            2
        );
        assert_eq!(
            draw_phase_manager
                .drawables_for_phase(DrawPhase::Opaque)
                .len(),
            1
        );
    }
}
