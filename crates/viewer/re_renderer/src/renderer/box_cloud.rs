//! Box renderer for efficient rendering of large numbers of axis-aligned boxes.
//!
//! How it works:
//! =================
//! Boxes are rendered as procedurally generated geometry with vertices created in the vertex shader.
//! Uploaded are only the data for the actual boxes (center + half_size + color), no vertex buffer!
//!
//! Like with the `super::point_cloud::PointCloudRenderer`, we're rendering all boxes in a single draw call.
//! Each box is rendered as 12 triangles (2 per face * 6 faces) = 36 vertices.
//!
//! For WebGL compatibility, data is uploaded as textures. Color is stored in a separate srgb texture, meaning
//! that srgb->linear conversion happens on texture load.

use std::{num::NonZeroU64, ops::Range};

use crate::{
    BoxCloudBuilder, DebugLabel, DepthOffset, DrawableCollector, OutlineMaskPreference,
    allocator::create_and_fill_uniform_buffer_batch,
    draw_phases::{DrawPhase, OutlineMaskProcessor, PickingLayerObjectId, PickingLayerProcessor},
    include_shader_module,
    renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo},
    wgpu_resources::GpuRenderPipelinePoolAccessor,
};
use bitflags::bitflags;
use enumset::{EnumSet, enum_set};
use itertools::Itertools as _;
use smallvec::smallvec;

use crate::{
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
    },
};

use super::{DrawData, DrawError, RenderContext, Renderer};

bitflags! {
    /// Property flags for a box batch
    ///
    /// Needs to be kept in sync with `box_cloud.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BoxCloudBatchFlags : u32 {
        /// If true, we shade all boxes in the batch with lighting.
        const FLAG_ENABLE_SHADING = 0b0001;
    }
}

pub mod gpu_data {
    use crate::{draw_phases::PickingLayerObjectId, wgpu_buffer_types};

    // Box data is stored as Vec4s in texture.
    // Each box uses 2 texels:
    // - Texel 0: (center.x, center.y, center.z, half_size.x)
    // - Texel 1: (half_size.y, half_size.z, 0, 0)

    /// Uniform buffer that changes once per draw data rendering.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DrawDataUniformBuffer {
        pub edge_radius_boost_in_ui_points: wgpu_buffer_types::F32RowPadded,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }

    /// Uniform buffer that changes for every batch of boxes.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BatchUniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,

        pub flags: u32, // BoxCloudBatchFlags
        pub depth_offset: f32,
        pub _row_padding: [f32; 2],

        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub picking_object_id: PickingLayerObjectId,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 6],
    }
}

/// Internal, ready to draw representation of [`BoxCloudBatchInfo`]
#[derive(Clone)]
struct BoxCloudBatch {
    bind_group: GpuBindGroup,
    vertex_range: Range<u32>,
    active_phases: EnumSet<DrawPhase>,
}

/// A box cloud drawing operation.
/// Expected to be recreated every frame.
#[derive(Clone)]
pub struct BoxCloudDrawData {
    bind_group_all_boxes: Option<GpuBindGroup>,
    bind_group_all_boxes_outline_mask: Option<GpuBindGroup>,
    batches: Vec<BoxCloudBatch>,
}

impl DrawData for BoxCloudDrawData {
    type Renderer = BoxCloudRenderer;

    fn collect_drawables(
        &self,
        _view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        for (batch_idx, batch) in self.batches.iter().enumerate() {
            collector.add_drawable(
                batch.active_phases,
                DrawDataDrawable {
                    // TODO(andreas): Don't have distance information yet. For now just always draw boxes last.
                    distance_sort_key: f32::MAX,
                    draw_data_payload: batch_idx as _,
                },
            );
        }
    }
}

/// Data that is valid for a batch of box cloud boxes.
pub struct BoxCloudBatchInfo {
    pub label: DebugLabel,

    /// Transformation applies to box centers
    pub world_from_obj: glam::Affine3A,

    /// Additional properties of this box cloud batch.
    pub flags: BoxCloudBatchFlags,

    /// Number of boxes covered by this batch.
    ///
    /// The batch will start with the next box after the one the previous batch ended with.
    pub box_count: u32,

    /// Optional outline mask setting for the entire batch.
    pub overall_outline_mask_ids: OutlineMaskPreference,

    /// Defines an outline mask for individual box ranges.
    ///
    /// Box ranges are relative within the current batch.
    ///
    /// Having many of these individual outline masks can be slow as they require each their own uniform buffer & draw call.
    pub additional_outline_mask_ids_vertex_ranges: Vec<(Range<u32>, OutlineMaskPreference)>,

    /// Picking object id that applies for the entire batch.
    pub picking_object_id: PickingLayerObjectId,

    /// Depth offset applied after projection.
    pub depth_offset: DepthOffset,
}

impl Default for BoxCloudBatchInfo {
    #[inline]
    fn default() -> Self {
        Self {
            label: DebugLabel::default(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: BoxCloudBatchFlags::FLAG_ENABLE_SHADING,
            box_count: 0,
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: Default::default(),
            depth_offset: 0,
        }
    }
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum BoxCloudDrawDataError {
    #[error("Failed to transfer data to the GPU: {0}")]
    FailedTransferringDataToGpu(#[from] crate::allocator::CpuWriteGpuReadError),
}

impl BoxCloudDrawData {
    /// Transforms and uploads box cloud data to be consumed by gpu.
    ///
    /// Try to bundle all boxes into a single draw data instance whenever possible.
    ///
    /// If no batches are passed, all boxes are assumed to be in a single batch with identity transform.
    pub fn new(builder: BoxCloudBuilder<'_>) -> Result<Self, BoxCloudDrawDataError> {
        re_tracing::profile_function!();

        let BoxCloudBuilder {
            ctx,
            position_halfsize_buffer,
            color_buffer,
            picking_instance_ids_buffer,
            batches,
            radius_boost_in_ui_points_for_outlines,
        } = builder;

        let box_renderer = ctx.renderer::<BoxCloudRenderer>();
        let batches = batches.as_slice();

        if position_halfsize_buffer.is_empty() {
            return Ok(Self {
                bind_group_all_boxes: None,
                bind_group_all_boxes_outline_mask: None,
                batches: Vec::new(),
            });
        }

        // position_halfsize_buffer stores 2 Vec4s per box, so divide by 2 to get box count
        debug_assert_eq!(
            position_halfsize_buffer.len() % 2,
            0,
            "position_halfsize_buffer length must be even (2 Vec4s per box)"
        );
        let num_boxes = position_halfsize_buffer.len() / 2;

        let fallback_batches = [BoxCloudBatchInfo {
            label: "fallback_batches".into(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: BoxCloudBatchFlags::empty(),
            box_count: num_boxes as _,
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: Default::default(),
            depth_offset: 0,
        }];
        let batches = if batches.is_empty() {
            &fallback_batches
        } else {
            batches
        };

        let position_halfsize_texture = position_halfsize_buffer.finish(
            wgpu::TextureFormat::Rgba32Float,
            "BoxCloudDrawData::position_halfsize_texture",
        )?;
        let color_texture = color_buffer.finish(
            wgpu::TextureFormat::Rgba8UnormSrgb,
            "BoxCloudDrawData::color_texture",
        )?;
        let picking_instance_id_texture = picking_instance_ids_buffer.finish(
            wgpu::TextureFormat::Rg32Uint,
            "BoxCloudDrawData::picking_instance_id_texture",
        )?;

        let draw_data_uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "BoxCloudDrawData::DrawDataUniformBuffer".into(),
            [
                gpu_data::DrawDataUniformBuffer {
                    edge_radius_boost_in_ui_points: 0.0.into(),
                    end_padding: Default::default(),
                },
                gpu_data::DrawDataUniformBuffer {
                    edge_radius_boost_in_ui_points: radius_boost_in_ui_points_for_outlines.into(),
                    end_padding: Default::default(),
                },
            ]
            .into_iter(),
        );
        let (draw_data_uniform_buffer_bindings_normal, draw_data_uniform_buffer_bindings_outline) =
            draw_data_uniform_buffer_bindings
                .into_iter()
                .collect_tuple()
                .unwrap();

        let mk_bind_group = |label, draw_data_uniform_buffer_binding| {
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label,
                    entries: smallvec![
                        BindGroupEntry::DefaultTextureView(position_halfsize_texture.handle),
                        BindGroupEntry::DefaultTextureView(color_texture.handle),
                        BindGroupEntry::DefaultTextureView(picking_instance_id_texture.handle),
                        draw_data_uniform_buffer_binding,
                    ],
                    layout: box_renderer.bind_group_layout_all_boxes,
                },
            )
        };

        let bind_group_all_boxes = mk_bind_group(
            "BoxCloudDrawData::bind_group_all_boxes".into(),
            draw_data_uniform_buffer_bindings_normal,
        );
        let bind_group_all_boxes_outline_mask = mk_bind_group(
            "BoxCloudDrawData::bind_group_all_boxes_outline_mask".into(),
            draw_data_uniform_buffer_bindings_outline,
        );

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "box batch uniform buffers".into(),
                batches
                    .iter()
                    .map(|batch_info| gpu_data::BatchUniformBuffer {
                        world_from_obj: batch_info.world_from_obj.into(),
                        flags: batch_info.flags.bits(),
                        outline_mask_ids: batch_info
                            .overall_outline_mask_ids
                            .0
                            .unwrap_or_default()
                            .into(),
                        picking_object_id: batch_info.picking_object_id,
                        depth_offset: batch_info.depth_offset as f32,

                        _row_padding: [0.0, 0.0],
                        end_padding: Default::default(),
                    }),
            );

            // Generate additional "micro batches" for each box range that has a unique outline setting.
            let mut uniform_buffer_bindings_mask_only_batches =
                create_and_fill_uniform_buffer_batch(
                    ctx,
                    "box batch uniform buffers - mask only".into(),
                    batches
                        .iter()
                        .flat_map(|batch_info| {
                            batch_info
                                .additional_outline_mask_ids_vertex_ranges
                                .iter()
                                .map(|(_, mask)| gpu_data::BatchUniformBuffer {
                                    world_from_obj: batch_info.world_from_obj.into(),
                                    flags: batch_info.flags.bits(),
                                    outline_mask_ids: mask.0.unwrap_or_default().into(),
                                    picking_object_id: batch_info.picking_object_id,
                                    depth_offset: batch_info.depth_offset as f32,

                                    _row_padding: [0.0, 0.0],
                                    end_padding: Default::default(),
                                })
                        })
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
                .into_iter();

            let mut start_box_for_next_batch = 0;
            for (batch_info, uniform_buffer_binding) in
                batches.iter().zip(uniform_buffer_bindings.into_iter())
            {
                let box_range_end = start_box_for_next_batch + batch_info.box_count;
                let mut active_phases = enum_set![DrawPhase::Opaque | DrawPhase::PickingLayer];
                // Does the entire batch participate in the outline mask phase?
                if batch_info.overall_outline_mask_ids.is_some() {
                    active_phases.insert(DrawPhase::OutlineMask);
                }

                batches_internal.push(box_renderer.create_box_cloud_batch(
                    ctx,
                    batch_info.label.clone(),
                    uniform_buffer_binding,
                    start_box_for_next_batch..box_range_end,
                    active_phases,
                ));

                for (range, _) in &batch_info.additional_outline_mask_ids_vertex_ranges {
                    let range = (range.start + start_box_for_next_batch)
                        ..(range.end + start_box_for_next_batch);
                    batches_internal.push(box_renderer.create_box_cloud_batch(
                        ctx,
                        format!("{:?} outline-only {:?}", batch_info.label, range).into(),
                        uniform_buffer_bindings_mask_only_batches.next().unwrap(),
                        range.clone(),
                        enum_set![DrawPhase::OutlineMask],
                    ));
                }

                start_box_for_next_batch = box_range_end;

                // Should happen only if the number of boxes was clamped.
                if start_box_for_next_batch >= num_boxes as u32 {
                    break;
                }
            }
        }

        Ok(Self {
            bind_group_all_boxes: Some(bind_group_all_boxes),
            bind_group_all_boxes_outline_mask: Some(bind_group_all_boxes_outline_mask),
            batches: batches_internal,
        })
    }
}

pub struct BoxCloudRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout_all_boxes: GpuBindGroupLayoutHandle,
    bind_group_layout_batch: GpuBindGroupLayoutHandle,
}

impl BoxCloudRenderer {
    fn create_box_cloud_batch(
        &self,
        ctx: &RenderContext,
        label: DebugLabel,
        uniform_buffer_binding: BindGroupEntry,
        box_range: Range<u32>,
        active_phases: EnumSet<DrawPhase>,
    ) -> BoxCloudBatch {
        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label,
                entries: smallvec![uniform_buffer_binding],
                layout: self.bind_group_layout_batch,
            },
        );

        BoxCloudBatch {
            bind_group,
            // Each box is 36 vertices (12 triangles * 3 vertices)
            vertex_range: (box_range.start * 36)..(box_range.end * 36),
            active_phases,
        }
    }
}

impl Renderer for BoxCloudRenderer {
    type RendererDrawData = BoxCloudDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let bind_group_layout_all_boxes = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "BoxCloudRenderer::bind_group_layout_all_boxes".into(),
                entries: vec![
                    // Position + half-size texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Color texture
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
                    // Picking instance ID texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Draw data uniform buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(std::mem::size_of::<
                                gpu_data::DrawDataUniformBuffer,
                            >() as _),
                        },
                        count: None,
                    },
                ],
            },
        );

        let bind_group_layout_batch = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "BoxCloudRenderer::bind_group_layout_batch".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(std::mem::size_of::<
                            gpu_data::BatchUniformBuffer,
                        >() as _),
                    },
                    count: None,
                }],
            },
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "BoxCloudRenderer::pipeline_layout".into(),
                entries: vec![
                    ctx.global_bindings.layout,
                    bind_group_layout_all_boxes,
                    bind_group_layout_batch,
                ],
            },
        );

        let shader_module = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/box_cloud.wgsl"),
        );

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "BoxCloudRenderer::render_pipeline_color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE),
            multisample: ViewBuilder::main_target_default_msaa_state(ctx.render_config(), false),
        };

        let render_pipeline_color =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color);
        let render_pipeline_picking_layer = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "BoxCloudRenderer::render_pipeline_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color.clone()
            },
        );
        let render_pipeline_outline_mask = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "BoxCloudRenderer::render_pipeline_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
                ..render_pipeline_desc_color
            },
        );

        Self {
            render_pipeline_color,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout_all_boxes,
            bind_group_layout_batch,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        if draw_instructions.is_empty() {
            return Ok(());
        }

        re_tracing::profile_function!();

        let pipeline_handle = match phase {
            DrawPhase::Opaque => self.render_pipeline_color,
            DrawPhase::PickingLayer => self.render_pipeline_picking_layer,
            DrawPhase::OutlineMask => self.render_pipeline_outline_mask,
            _ => unreachable!("We should only be called for Opaque, PickingLayer and OutlineMask"),
        };
        let pipeline = render_pipelines.get(pipeline_handle)?;

        pass.set_pipeline(pipeline);

        for instruction in draw_instructions {
            let box_cloud_draw_data = &instruction.draw_data;

            if box_cloud_draw_data.batches.is_empty() {
                continue;
            }

            let bind_group_all_boxes = match phase {
                DrawPhase::OutlineMask => &box_cloud_draw_data.bind_group_all_boxes_outline_mask,
                _ => &box_cloud_draw_data.bind_group_all_boxes,
            };
            if let Some(bind_group_all_boxes) = bind_group_all_boxes {
                pass.set_bind_group(1, bind_group_all_boxes, &[]);
            } else {
                continue;
            }

            for drawable in instruction.drawables.iter() {
                let batch = &box_cloud_draw_data.batches[drawable.draw_data_payload as usize];
                pass.set_bind_group(2, &batch.bind_group, &[]);
                pass.draw(batch.vertex_range.clone(), 0..1);
            }
        }

        Ok(())
    }
}
