use re_entity_db::InstancePathHash;
use re_log_types::Instance;
use re_renderer::{
    renderer::MeshInstance, LineDrawableBuilder, PickingLayerInstanceId, RenderContext,
};
use re_types::{
    archetypes::Boxes3D,
    components::{ClassId, Color, FillMode, HalfSize3D, KeypointId, Radius, ShowLabels, Text},
    ArrowString, Loggable as _,
};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    instance_hash_conversions::picking_layer_id_from_instance_path_hash, proc_mesh,
    view_kind::SpatialSpaceViewKind,
};

use super::{
    entity_iterator::clamped_or_nothing, filter_visualizable_3d_entities,
    process_annotation_and_keypoint_slices, process_color_slice, process_labels_3d,
    process_radius_slice, utilities::LabeledBatch, SpatialViewVisualizerData,
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct Boxes3DVisualizer(SpatialViewVisualizerData);

impl Default for Boxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Boxes3DVisualizer {
    #[allow(clippy::too_many_arguments)]
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        mesh_instances: &mut Vec<MeshInstance>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Boxes3DComponentData<'a>>,
        render_ctx: &RenderContext,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let entity_path = ctx.target_entity_path;

        for data in data {
            if data.half_sizes.is_empty() {
                continue;
            }

            // Draw as many boxes as we have max(instances, boxes), all components get repeated over that number.
            // TODO(#7026): We should formalize this kind of hybrid joining better.
            let num_instances = data
                .half_sizes
                .len()
                .max(ent_context.transform_info.reference_from_instances.len());
            let half_sizes = clamped_or_nothing(data.half_sizes, num_instances);

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                std::iter::repeat(glam::Vec3::ZERO).take(num_instances),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii =
                process_radius_slice(entity_path, num_instances, data.radii, Radius::default());
            let colors =
                process_color_slice(ctx, self, num_instances, &annotation_infos, data.colors);

            let mut line_batch = line_builder
                .batch("boxes3d")
                .depth_offset(ent_context.depth_offset)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut world_space_bounding_box = re_math::BoundingBox::NOTHING;

            let world_from_instances = ent_context
                .transform_info
                .clamped_reference_from_instances();

            for (instance_index, (half_size, world_from_instance, radius, &color)) in
                itertools::izip!(half_sizes, world_from_instances, radii, &colors).enumerate()
            {
                let instance = Instance::from(instance_index as u64);
                let proc_mesh_key = proc_mesh::ProcMeshKey::Cube;

                let world_from_instance = world_from_instance
                    * glam::Affine3A::from_scale(glam::Vec3::from(*half_size) * 2.0);
                world_space_bounding_box = world_space_bounding_box.union(
                    proc_mesh_key
                        .simple_bounding_box()
                        .transform_affine3(&world_from_instance),
                );

                match data.fill_mode {
                    FillMode::DenseWireframe | FillMode::MajorWireframe => {
                        let box3d = line_batch
                            .add_box_outline_from_transform(world_from_instance)
                            .color(color)
                            .radius(radius)
                            .picking_instance_id(PickingLayerInstanceId(instance_index as _));

                        if let Some(outline_mask_ids) =
                            ent_context.highlight.instances.get(&instance)
                        {
                            box3d.outline_mask_ids(*outline_mask_ids);
                        }
                    }
                    FillMode::Solid => {
                        let Some(solid_mesh) =
                            ctx.viewer_ctx.cache.entry(|c: &mut proc_mesh::SolidCache| {
                                c.entry(proc_mesh_key, render_ctx)
                            })
                        else {
                            return Err(SpaceViewSystemExecutionError::DrawDataCreationError(
                                "Failed to allocate solid mesh".into(),
                            ));
                        };

                        mesh_instances.push(MeshInstance {
                            gpu_mesh: solid_mesh.gpu_mesh,
                            mesh: None,
                            world_from_mesh: world_from_instance,
                            outline_mask_ids: ent_context.highlight.index_outline_mask(instance),
                            picking_layer_id: picking_layer_id_from_instance_path_hash(
                                InstancePathHash::instance(entity_path, instance),
                            ),
                            additive_tint: color,
                        });
                    }
                }
            }

            self.0
                .bounding_boxes
                .push((entity_path.hash(), world_space_bounding_box));

            self.0.ui_labels.extend(process_labels_3d(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: world_space_bounding_box.center(),
                    instance_positions: ent_context
                        .transform_info
                        .clamped_reference_from_instances()
                        .map(|t| t.translation.into()),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels,
                    annotation_infos: &annotation_infos,
                },
                glam::Affine3A::IDENTITY,
            ));
        }

        Ok(())
    }
}

// ---

struct Boxes3DComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSize3D],

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Boxes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes3D".into()
    }
}

impl VisualizerSystem for Boxes3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Boxes3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let mut line_builder = LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        // Collects solid (that is, triangles rather than wireframe) instances to be drawn.
        //
        // Why do we draw solid surfaces using instanced meshes and wireframes using instances?
        // No good reason; only, those are the types of renderers that have been implemented so far.
        // This code should be revisited with an eye on performance.
        let mut solid_instances: Vec<MeshInstance> = Vec::new();

        use super::entity_iterator::{iter_primitive_array, process_archetype};
        process_archetype::<Self, Boxes3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;

                let Some(all_half_size_chunks) = results.get_required_chunks(&HalfSize3D::name())
                else {
                    return Ok(());
                };

                let num_boxes: usize = all_half_size_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_primitive_array::<3, f32>(&HalfSize3D::name()))
                    .map(|vectors| vectors.len())
                    .sum();
                if num_boxes == 0 {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_half_sizes_indexed = iter_primitive_array::<3, f32>(
                    &all_half_size_chunks,
                    timeline,
                    HalfSize3D::name(),
                );
                let all_colors = results.iter_as(timeline, Color::name());
                let all_radii = results.iter_as(timeline, Radius::name());
                let all_labels = results.iter_as(timeline, Text::name());
                let all_class_ids = results.iter_as(timeline, ClassId::name());
                let all_keypoint_ids = results.iter_as(timeline, KeypointId::name());
                let all_show_labels = results.iter_as(timeline, ShowLabels::name());

                // Deserialized because it's a union.
                let all_fill_modes = results.iter_as(timeline, FillMode::name());
                // fill mode is currently a non-repeated component
                let fill_mode: FillMode = all_fill_modes
                    .component::<FillMode>()
                    .next()
                    .and_then(|(_, fill_modes)| fill_modes.as_slice().first().copied())
                    .unwrap_or_default();

                match fill_mode {
                    FillMode::DenseWireframe | FillMode::MajorWireframe => {
                        // Each box consists of 12 independent lines with 2 vertices each.
                        line_builder.reserve_strips(num_boxes * 12)?;
                        line_builder.reserve_vertices(num_boxes * 12 * 2)?;
                    }
                    FillMode::Solid => {
                        // No lines.
                    }
                }

                let data = re_query::range_zip_1x6(
                    all_half_sizes_indexed,
                    all_colors.primitive::<u32>(),
                    all_radii.primitive::<f32>(),
                    all_labels.string(),
                    all_class_ids.primitive::<u16>(),
                    all_keypoint_ids.primitive::<u16>(),
                    all_show_labels.component::<ShowLabels>(),
                )
                .map(
                    |(
                        _index,
                        half_sizes,
                        colors,
                        radii,
                        labels,
                        class_ids,
                        keypoint_ids,
                        show_labels,
                    )| {
                        Boxes3DComponentData {
                            half_sizes: bytemuck::cast_slice(half_sizes),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            // fill mode is currently a non-repeated component
                            fill_mode,
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            keypoint_ids: keypoint_ids
                                .map_or(&[], |keypoint_ids| bytemuck::cast_slice(keypoint_ids)),
                            show_labels: show_labels.unwrap_or_default().first().copied(),
                        }
                    },
                );

                self.process_data(
                    ctx,
                    &mut line_builder,
                    &mut solid_instances,
                    view_query,
                    spatial_ctx,
                    data,
                    render_ctx,
                )?;

                Ok(())
            },
        )?;

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

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Color> for Boxes3DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

re_viewer_context::impl_component_fallback_provider!(Boxes3DVisualizer => [Color]);
