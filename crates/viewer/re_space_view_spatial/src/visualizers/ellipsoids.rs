use re_entity_db::{EntityPath, InstancePathHash};
use re_log_types::Instance;
use re_query::range_zip_1x7;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, RenderContext};
use re_types::{
    archetypes::Ellipsoids,
    components::{ClassId, Color, HalfSize3D, KeypointId, Position3D, Radius, Rotation3D, Text},
};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    ResolvedAnnotationInfos, SpaceViewSystemExecutionError, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    proc_mesh::{ProcMeshKey, WireframeCache},
    view_kind::SpatialSpaceViewKind,
    visualizers::{UiLabel, UiLabelTarget},
};

use super::{
    entity_iterator::clamped, filter_visualizable_3d_entities,
    process_annotation_and_keypoint_slices, process_color_slice, process_radius_slice,
    SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct EllipsoidsVisualizer(SpatialViewVisualizerData);

impl Default for EllipsoidsVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl EllipsoidsVisualizer {
    fn process_labels<'a>(
        entity_path: &'a EntityPath,
        half_sizes: &'a [HalfSize3D],
        centers: impl Iterator<Item = &'a Position3D> + 'a,
        labels: &'a [Text],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_entity: glam::Affine3A,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        let labels = clamped(labels, half_sizes.len());
        let centers = centers.chain(std::iter::repeat(&Position3D::ZERO));
        itertools::izip!(annotation_infos.iter(), centers, labels, colors)
            .enumerate()
            .filter_map(move |(i, (annotation_info, center, label, color))| {
                let label = annotation_info.label(Some(label.as_str()));
                label.map(|label| UiLabel {
                    text: label,
                    color: *color,
                    target: UiLabelTarget::Position3D(
                        world_from_entity.transform_point3(center.0.into()),
                    ),
                    labeled_instance: InstancePathHash::instance(
                        entity_path,
                        Instance::from(i as u64),
                    ),
                })
            })
    }

    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = EllipsoidsComponentData<'a>>,
        render_ctx: &RenderContext,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.half_sizes.len();
            if num_instances == 0 {
                continue;
            }

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                data.half_sizes.iter().map(|_| glam::Vec3::ZERO),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii = process_radius_slice(
                entity_path,
                num_instances,
                data.line_radii,
                Radius::default(),
            );
            let colors =
                process_color_slice(ctx, self, num_instances, &annotation_infos, data.colors);

            let centers = clamped(data.centers, num_instances);
            self.0.ui_labels.extend(Self::process_labels(
                entity_path,
                data.half_sizes,
                centers,
                data.labels,
                &colors,
                &annotation_infos,
                ent_context.world_from_entity,
            ));

            let mut line_batch = line_builder
                .batch("ellipsoids")
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut bounding_box = macaw::BoundingBox::nothing();

            let centers =
                clamped(data.centers, num_instances).chain(std::iter::repeat(&Position3D::ZERO));
            let rotations = clamped(data.rotations, num_instances)
                .chain(std::iter::repeat(&Rotation3D::IDENTITY));
            for (i, (half_size, &center, rotation, radius, color)) in
                itertools::izip!(data.half_sizes, centers, rotations, radii, colors).enumerate()
            {
                let transform = glam::Affine3A::from_scale_rotation_translation(
                    glam::Vec3::from(*half_size),
                    rotation.0.into(),
                    center.into(),
                );

                // TODO(kpreid): subdivisions should be configurable, and possibly dynamic based on
                // either world size or screen size (depending on application).
                let subdivisions = 4;

                let Some(sphere_mesh) = ctx.viewer_ctx.cache.entry(|c: &mut WireframeCache| {
                    c.entry(ProcMeshKey::Sphere { subdivisions }, render_ctx)
                }) else {
                    // TODO(kpreid): Should this be just returning nothing instead?
                    // If we do, there won't be any error report, just missing data.

                    return Err(SpaceViewSystemExecutionError::DrawDataCreationError(
                        "Failed to allocate wireframe mesh".into(),
                    ));
                };

                bounding_box = bounding_box.union(sphere_mesh.bbox.transform_affine3(&transform));

                for strip in &sphere_mesh.line_strips {
                    let box3d = line_batch
                        .add_strip(strip.iter().map(|&point| transform.transform_point3(point)))
                        .color(color)
                        .radius(radius)
                        .picking_instance_id(PickingLayerInstanceId(i as _));

                    if let Some(outline_mask_ids) = ent_context
                        .highlight
                        .instances
                        .get(&Instance::from(i as u64))
                    {
                        box3d.outline_mask_ids(*outline_mask_ids);
                    }
                }
            }

            self.0.add_bounding_box(
                entity_path.hash(),
                bounding_box,
                ent_context.world_from_entity,
            );
        }

        Ok(())
    }
}

// ---

struct EllipsoidsComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSize3D],

    // Clamped to edge
    centers: &'a [Position3D],
    rotations: &'a [Rotation3D],
    colors: &'a [Color],
    line_radii: &'a [Radius],
    labels: &'a [Text],
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],
}

impl IdentifiedViewSystem for EllipsoidsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Ellipsoids".into()
    }
}

impl VisualizerSystem for EllipsoidsVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Ellipsoids>()
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

        // TODO(kpreid): Should be using instanced meshes kept in GPU buffers
        // instead of this immediate-mode strategy that copies every vertex every frame.
        let mut line_builder = LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype::<Self, Ellipsoids, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let half_sizes = match results.get_required_component_dense::<HalfSize3D>(resolver)
                {
                    Some(vectors) => vectors?,
                    _ => return Ok(()),
                };

                let num_ellipsoids = half_sizes
                    .range_indexed()
                    .map(|(_, vectors)| vectors.len())
                    .sum::<usize>();
                if num_ellipsoids == 0 {
                    return Ok(());
                }

                // Ideally we would reserve space here, but we don't know the mesh subdivision yet,
                // and this will become moot when we switch to instanced meshes.
                // line_builder.reserve_strips(num_ellipsoids * sphere_mesh.line_strips.len())?;
                // line_builder.reserve_vertices(num_ellipsoids * sphere_mesh.vertex_count)?;

                let centers = results.get_or_empty_dense(resolver)?;
                let rotations = results.get_or_empty_dense(resolver)?;
                let colors = results.get_or_empty_dense(resolver)?;
                let line_radii = results.get_or_empty_dense(resolver)?;
                let labels = results.get_or_empty_dense(resolver)?;
                let class_ids = results.get_or_empty_dense(resolver)?;
                let keypoint_ids = results.get_or_empty_dense(resolver)?;

                let data = range_zip_1x7(
                    half_sizes.range_indexed(),
                    centers.range_indexed(),
                    rotations.range_indexed(),
                    colors.range_indexed(),
                    line_radii.range_indexed(),
                    labels.range_indexed(),
                    class_ids.range_indexed(),
                    keypoint_ids.range_indexed(),
                )
                .map(
                    |(
                        _index,
                        half_sizes,
                        centers,
                        rotations,
                        colors,
                        line_radii,
                        labels,
                        class_ids,
                        keypoint_ids,
                    )| {
                        EllipsoidsComponentData {
                            half_sizes,
                            centers: centers.unwrap_or_default(),
                            rotations: rotations.unwrap_or_default(),
                            colors: colors.unwrap_or_default(),
                            line_radii: line_radii.unwrap_or_default(),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids.unwrap_or_default(),
                            keypoint_ids: keypoint_ids.unwrap_or_default(),
                        }
                    },
                );

                self.process_data(
                    ctx,
                    &mut line_builder,
                    view_query,
                    spatial_ctx,
                    data,
                    render_ctx,
                )?;

                Ok(())
            },
        )?;

        Ok(vec![(line_builder.into_draw_data()?.into())])
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

impl TypedComponentFallbackProvider<Color> for EllipsoidsVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

re_viewer_context::impl_component_fallback_provider!(EllipsoidsVisualizer => [Color]);
