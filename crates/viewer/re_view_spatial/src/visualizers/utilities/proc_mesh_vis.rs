use re_entity_db::InstancePathHash;
use re_log_types::Instance;
use re_renderer::renderer::{GpuMeshInstance, LineStripFlags};
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, RenderContext};
use re_types::ArchetypeName;
use re_types::components::{self, FillMode};
use re_view::{clamped_or_nothing, process_annotation_slices, process_color_slice};
use re_viewer_context::{
    QueryContext, TypedComponentFallbackProvider, ViewQuery, ViewSystemExecutionError,
};

use crate::contexts::SpatialSceneEntityContext;
use crate::proc_mesh::{self, ProcMeshKey};
use crate::visualizers::{
    SpatialViewVisualizerData, process_labels_3d, process_radius_slice, utilities::LabeledBatch,
};

#[cfg(doc)]
use re_viewer_context::VisualizerSystem;

/// To be used within the scope of a single [`VisualizerSystem::execute()`] call
/// when the visualizer wishes to draw batches of [`ProcMeshKey`] meshes.
pub struct ProcMeshDrawableBuilder<'ctx, Fb> {
    /// Bounding box and label info here will be updated by the drawn batches.
    pub data: &'ctx mut SpatialViewVisualizerData,
    pub fallback: &'ctx Fb,

    /// Accumulates lines to render.
    /// TODO(kpreid): Should be using instanced meshes kept in GPU buffers
    /// instead of this immediate-mode strategy that copies every vertex every frame.
    pub line_builder: re_renderer::LineDrawableBuilder<'ctx>,
    pub line_batch_debug_label: re_renderer::DebugLabel,

    /// Accumulates triangle mesh instances to render.
    pub solid_instances: Vec<GpuMeshInstance>,

    pub query: &'ctx ViewQuery<'ctx>,
    pub render_ctx: &'ctx RenderContext,
}

/// A [batch] of instances to draw. This struct is just arguments to
/// [`ProcMeshDrawableBuilder::add_batch()`].
///
/// TODO(#7026): Document how the number of instances is derived from this data.
///
/// [batch]: https://rerun.io/docs/concepts/batches
pub struct ProcMeshBatch<'a, IMesh, IFill> {
    pub half_sizes: &'a [components::HalfSize3D],

    /// Iterator of meshes. Must be at least as long as `half_sizes`.
    pub meshes: IMesh,

    /// Iterator of mesh fill modes. Must be at least as long as `half_sizes`.
    pub fill_modes: IFill,

    pub line_radii: &'a [components::Radius],
    pub colors: &'a [components::Color],
    pub labels: &'a [re_types::ArrowString],
    pub show_labels: Option<components::ShowLabels>,
    pub class_ids: &'a [components::ClassId],
}

impl<'ctx, Fb> ProcMeshDrawableBuilder<'ctx, Fb>
where
    Fb: TypedComponentFallbackProvider<components::ShowLabels>
        + TypedComponentFallbackProvider<components::Color>,
{
    pub fn new(
        data: &'ctx mut SpatialViewVisualizerData,
        render_ctx: &'ctx re_renderer::RenderContext,
        view_query: &'ctx ViewQuery<'ctx>,
        line_batch_debug_label: impl Into<re_renderer::DebugLabel>,
        fallback: &'ctx Fb,
    ) -> Self {
        let mut line_builder = LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        ProcMeshDrawableBuilder {
            data,
            fallback,
            line_builder,
            line_batch_debug_label: line_batch_debug_label.into(),
            solid_instances: Vec::new(),
            query: view_query,
            render_ctx,
        }
    }

    /// Add a batch of data to be drawn.
    pub fn add_batch(
        &mut self,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        archetype_name: ArchetypeName,
        constant_instance_transform: glam::Affine3A,
        batch: ProcMeshBatch<'_, impl Iterator<Item = ProcMeshKey>, impl Iterator<Item = FillMode>>,
    ) -> Result<(), ViewSystemExecutionError> {
        let entity_path = query_context.target_entity_path;

        if batch.half_sizes.is_empty() {
            return Ok(());
        }

        // Draw as many boxes as we have max(instances, boxes), all components get repeated over that number.
        // TODO(#7026): We should formalize this kind of hybrid joining better.

        let reference_from_instances = ent_context
            .transform_info
            .reference_from_instances(archetype_name);
        let num_instances = batch.half_sizes.len().max(reference_from_instances.len());
        let half_sizes = clamped_or_nothing(batch.half_sizes, num_instances);

        let annotation_infos = process_annotation_slices(
            self.query.latest_at,
            num_instances,
            batch.class_ids,
            &ent_context.annotations,
        );

        // Has not custom fallback for radius, so we use the default.
        // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
        let line_radii = process_radius_slice(
            entity_path,
            num_instances,
            batch.line_radii,
            components::Radius::default(),
        );
        let colors = process_color_slice(
            query_context,
            self.fallback,
            num_instances,
            &annotation_infos,
            batch.colors,
        );

        let mut line_batch = self
            .line_builder
            .batch(self.line_batch_debug_label.clone())
            .depth_offset(ent_context.depth_offset)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

        let mut world_space_bounding_box = macaw::BoundingBox::nothing();

        let world_from_instances = reference_from_instances
            .iter()
            .chain(std::iter::repeat(reference_from_instances.last()))
            .copied();

        let mut num_instances = 0;
        for (
            instance_index,
            (half_size, world_from_instance, radius, &color, proc_mesh_key, fill_mode),
        ) in itertools::izip!(
            half_sizes,
            world_from_instances,
            line_radii,
            colors.iter(),
            batch.meshes,
            batch.fill_modes
        )
        .enumerate()
        {
            let instance = Instance::from(instance_index as u64);
            num_instances = instance_index + 1;

            let world_from_instance = world_from_instance
                * glam::Affine3A::from_scale(glam::Vec3::from(*half_size))
                * constant_instance_transform;
            world_space_bounding_box = world_space_bounding_box.union(
                proc_mesh_key
                    .simple_bounding_box()
                    .transform_affine3(&world_from_instance),
            );

            match fill_mode {
                FillMode::MajorWireframe | FillMode::DenseWireframe => {
                    let Some(wireframe_mesh) = query_context.viewer_ctx.store_context.caches.entry(
                        |c: &mut proc_mesh::WireframeCache| c.entry(proc_mesh_key, self.render_ctx),
                    ) else {
                        return Err(ViewSystemExecutionError::DrawDataCreationError(
                            "Failed to allocate wireframe mesh".into(),
                        ));
                    };

                    for strip in &wireframe_mesh.line_strips {
                        let strip_builder = line_batch
                            .add_strip(
                                strip
                                    .iter()
                                    .map(|&point| world_from_instance.transform_point3(point)),
                            )
                            .color(color)
                            .radius(radius)
                            .picking_instance_id(PickingLayerInstanceId(instance_index as _))
                            // Looped lines should be connected with rounded corners.
                            .flags(LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS);

                        if let Some(outline_mask_ids) = ent_context
                            .highlight
                            .instances
                            .get(&Instance::from(instance_index as u64))
                        {
                            // Not using ent_context.highlight.index_outline_mask() because
                            // that's already handled when the builder was created.
                            strip_builder.outline_mask_ids(*outline_mask_ids);
                        }
                    }
                }
                FillMode::Solid => {
                    let Some(solid_mesh) = query_context.viewer_ctx.store_context.caches.entry(
                        |c: &mut proc_mesh::SolidCache| c.entry(proc_mesh_key, self.render_ctx),
                    ) else {
                        return Err(ViewSystemExecutionError::DrawDataCreationError(
                            "Failed to allocate solid mesh".into(),
                        ));
                    };

                    self.solid_instances.push(GpuMeshInstance {
                        gpu_mesh: solid_mesh.gpu_mesh,
                        world_from_mesh: world_from_instance,
                        outline_mask_ids: ent_context.highlight.index_outline_mask(instance),
                        picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                            InstancePathHash::instance(entity_path, instance),
                        ),
                        additive_tint: color,
                    });
                }
            }
        }

        self.data
            .bounding_boxes
            .push((entity_path.hash(), world_space_bounding_box));

        self.data.ui_labels.extend(process_labels_3d(
            LabeledBatch {
                entity_path,
                num_instances,
                overall_position: world_space_bounding_box.center(),
                instance_positions: reference_from_instances
                    .iter()
                    .chain(std::iter::repeat(reference_from_instances.last()))
                    .map(|t| t.translation.into()),
                labels: batch.labels,
                colors: &colors,
                show_labels: batch
                    .show_labels
                    .unwrap_or_else(|| self.fallback.fallback_for(query_context)),
                annotation_infos: &annotation_infos,
            },
            glam::Affine3A::IDENTITY,
        ));

        Ok(())
    }

    /// Final operation. Produce the [`re_renderer::QueueableDrawData`] to actually be drawn.
    pub fn into_draw_data(
        self,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let Self {
            data: _,
            fallback: _,
            line_builder,
            line_batch_debug_label: _,
            solid_instances,
            query: _,
            render_ctx,
        } = self;
        let wireframe_draw_data: re_renderer::QueueableDrawData =
            line_builder.into_draw_data()?.into();

        let solid_draw_data: Option<re_renderer::QueueableDrawData> =
            match re_renderer::renderer::MeshDrawData::new(render_ctx, &solid_instances) {
                Ok(draw_data) => Some(draw_data.into()),
                Err(err) => {
                    re_log::error_once!(
                        "Failed to create mesh draw data from mesh instances: {err}"
                    );
                    None
                }
            };

        Ok([solid_draw_data, Some(wireframe_draw_data)]
            .into_iter()
            .flatten()
            .collect())
    }
}
