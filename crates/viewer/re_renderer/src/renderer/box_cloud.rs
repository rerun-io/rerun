//! Box cloud renderer for efficient rendering of large numbers of axis-aligned boxes.
//!
//! Uses instanced rendering similar to the mesh renderer:
//! - Static vertex buffer containing unit cube geometry (36 vertices)
//! - Instance buffer with per-box data (center, half-size, color, picking IDs)
//! - Single draw call per batch using GPU instancing
//!
//! This approach is appropriate for boxes because:
//! - All boxes share the same geometry (unlike point sprites)
//! - No per-box texture binding required (unlike rectangles)
//! - Standard GPU instancing path provides good performance and driver optimization

use std::ops::Range;

use crate::{
    BoxCloudBuilder, Color32, CpuWriteGpuReadError, DebugLabel, DepthOffset, DrawableCollector,
    OutlineMaskPreference, PickingLayerInstanceId,
    allocator::create_and_fill_uniform_buffer_batch,
    draw_phases::{DrawPhase, OutlineMaskProcessor, PickingLayerObjectId, PickingLayerProcessor},
    include_shader_module,
    renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BufferDesc, GpuBindGroup,
        GpuBindGroupLayoutHandle, GpuBuffer, GpuRenderPipelineHandle,
        GpuRenderPipelinePoolAccessor, PipelineLayoutDesc, RenderPipelineDesc,
        VertexBufferLayout,
    },
};
use bitflags::bitflags;
use enumset::{EnumSet, enum_set};
use smallvec::smallvec;

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
    use crate::{wgpu_buffer_types, wgpu_resources::VertexBufferLayout};

    use super::*;

    /// Vertex data for unit cube geometry.
    /// The cube is centered at origin with extent [-0.5, 0.5]³.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BoxVertex {
        pub position: [f32; 3],
        pub normal: [f32; 3],
    }

    impl BoxVertex {
        pub fn vertex_buffer_layout() -> VertexBufferLayout {
            VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: VertexBufferLayout::attributes_from_formats(
                    0,
                    [
                        wgpu::VertexFormat::Float32x3, // position
                        wgpu::VertexFormat::Float32x3, // normal
                    ]
                    .into_iter(),
                ),
            }
        }
    }

    /// Per-instance data in the instance buffer.
    /// Keep in sync with `box_cloud.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BoxInstanceData {
        pub center: [f32; 3],
        pub half_size_x: f32,
        pub half_size_yz: [f32; 2],
        pub color: Color32,
        pub picking_instance_id: PickingLayerInstanceId,
    }

    impl BoxInstanceData {
        pub fn vertex_buffer_layout() -> VertexBufferLayout {
            VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as _,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: VertexBufferLayout::attributes_from_formats(
                    2, // Start after BoxVertex attributes
                    [
                        wgpu::VertexFormat::Float32x3, // center
                        wgpu::VertexFormat::Float32,   // half_size_x
                        wgpu::VertexFormat::Float32x2, // half_size_yz
                        wgpu::VertexFormat::Unorm8x4,  // color
                        wgpu::VertexFormat::Uint32x2,  // picking_instance_id
                    ]
                    .into_iter(),
                ),
            }
        }
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
    instance_range: Range<u32>,
    active_phases: EnumSet<DrawPhase>,
}

/// A box cloud drawing operation.
/// Expected to be recreated every frame.
#[derive(Clone)]
pub struct BoxCloudDrawData {
    vertex_buffer: GpuBuffer,
    instance_buffer: Option<GpuBuffer>,
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
            instances,
            batches,
            ..
        } = builder;

        let box_renderer = ctx.renderer::<BoxCloudRenderer>();
        let batches = batches.as_slice();

        if instances.is_empty() {
            return Ok(Self {
                vertex_buffer: box_renderer.unit_cube_vertex_buffer.clone(),
                instance_buffer: None,
                batches: Vec::new(),
            });
        }

        let fallback_batches = [BoxCloudBatchInfo {
            label: "fallback_batches".into(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: BoxCloudBatchFlags::empty(),
            box_count: instances.len() as _,
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

        // Upload instance data
        let instance_buffer_size =
            (std::mem::size_of::<gpu_data::BoxInstanceData>() * instances.len()) as _;
        let instance_buffer = ctx.gpu_resources.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "BoxCloudDrawData::instance_buffer".into(),
                size: instance_buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );

        // Write instance data
        {
            let instance_data: Vec<gpu_data::BoxInstanceData> = instances
                .iter()
                .map(|inst| gpu_data::BoxInstanceData {
                    center: inst.center.into(),
                    half_size_x: inst.half_size.x,
                    half_size_yz: [inst.half_size.y, inst.half_size.z],
                    color: inst.color,
                    picking_instance_id: inst.picking_instance_id,
                })
                .collect();

            ctx.queue
                .write_buffer(&instance_buffer, 0, bytemuck::cast_slice(&instance_data));
        }

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "box batch uniform buffers".into(),
                batches.iter().map(|batch_info| gpu_data::BatchUniformBuffer {
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
                if start_box_for_next_batch >= instances.len() as u32 {
                    break;
                }
            }
        }

        Ok(Self {
            vertex_buffer: box_renderer.unit_cube_vertex_buffer.clone(),
            instance_buffer: Some(instance_buffer),
            batches: batches_internal,
        })
    }
}

pub struct BoxCloudRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
    unit_cube_vertex_buffer: GpuBuffer,
}

impl BoxCloudRenderer {
    fn create_box_cloud_batch(
        &self,
        ctx: &RenderContext,
        label: DebugLabel,
        uniform_buffer_binding: BindGroupEntry,
        instance_range: Range<u32>,
        active_phases: EnumSet<DrawPhase>,
    ) -> BoxCloudBatch {
        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label,
                entries: smallvec![uniform_buffer_binding],
                layout: self.bind_group_layout,
            },
        );

        BoxCloudBatch {
            bind_group,
            instance_range,
            active_phases,
        }
    }

    /// Creates vertex data for a unit cube centered at origin with extent [-0.5, 0.5]³.
    /// Returns 36 vertices (12 triangles, 2 per face).
    fn create_unit_cube_vertices() -> Vec<gpu_data::BoxVertex> {
        use gpu_data::BoxVertex;

        // Define 8 corners of the unit cube
        let corners = [
            [-0.5, -0.5, -0.5], // 0
            [0.5, -0.5, -0.5],  // 1
            [0.5, 0.5, -0.5],   // 2
            [-0.5, 0.5, -0.5],  // 3
            [-0.5, -0.5, 0.5],  // 4
            [0.5, -0.5, 0.5],   // 5
            [0.5, 0.5, 0.5],    // 6
            [-0.5, 0.5, 0.5],   // 7
        ];

        // Define 6 faces with their normals and vertex indices
        // Each face is two triangles (6 vertices)
        let faces = [
            // Front face (+Z)
            ([0.0, 0.0, 1.0], [4, 5, 6, 4, 6, 7]),
            // Back face (-Z)
            ([0.0, 0.0, -1.0], [1, 0, 3, 1, 3, 2]),
            // Right face (+X)
            ([1.0, 0.0, 0.0], [5, 1, 2, 5, 2, 6]),
            // Left face (-X)
            ([-1.0, 0.0, 0.0], [0, 4, 7, 0, 7, 3]),
            // Top face (+Y)
            ([0.0, 1.0, 0.0], [7, 6, 2, 7, 2, 3]),
            // Bottom face (-Y)
            ([0.0, -1.0, 0.0], [0, 1, 5, 0, 5, 4]),
        ];

        let mut vertices = Vec::with_capacity(36);
        for (normal, indices) in faces {
            for &idx in &indices {
                vertices.push(BoxVertex {
                    position: corners[idx],
                    normal,
                });
            }
        }

        vertices
    }
}

impl Renderer for BoxCloudRenderer {
    type RendererDrawData = BoxCloudDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "BoxCloudRenderer::bind_group_layout".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
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
                entries: vec![ctx.global_bindings.layout, bind_group_layout],
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
            vertex_buffers: smallvec![
                gpu_data::BoxVertex::vertex_buffer_layout(),
                gpu_data::BoxInstanceData::vertex_buffer_layout(),
            ],
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
                render_targets: smallvec![Some(
                    PickingLayerProcessor::PICKING_LAYER_FORMAT.into()
                )],
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

        // Create static vertex buffer for unit cube
        let cube_vertices = Self::create_unit_cube_vertices();
        let cube_vertex_buffer = ctx.gpu_resources.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "BoxCloudRenderer::unit_cube_vertex_buffer".into(),
                size: (std::mem::size_of::<gpu_data::BoxVertex>() * cube_vertices.len()) as _,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );
        ctx.queue.write_buffer(
            &cube_vertex_buffer,
            0,
            bytemuck::cast_slice(&cube_vertices),
        );

        Self {
            render_pipeline_color,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout,
            unit_cube_vertex_buffer: cube_vertex_buffer,
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

            // Set vertex buffer (shared by all batches)
            pass.set_vertex_buffer(0, box_cloud_draw_data.vertex_buffer.slice(..));

            // Set instance buffer (if any)
            if let Some(instance_buffer) = &box_cloud_draw_data.instance_buffer {
                pass.set_vertex_buffer(1, instance_buffer.slice(..));

                for drawable in instruction.drawables.iter() {
                    let batch = &box_cloud_draw_data.batches[drawable.draw_data_payload as usize];
                    pass.set_bind_group(1, &batch.bind_group, &[]);
                    // Draw 36 vertices (unit cube) per instance
                    pass.draw(0..36, batch.instance_range.clone());
                }
            }
        }

        Ok(())
    }
}
