use nohash_hasher::IntSet;

use re_log_types::{EntityPath, Instance};
use re_types::{
    Archetype, ComponentType,
    archetypes::{Points3D, Transform3D, TransformArrows3D},
    components::AxisLength,
};
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{contexts::TransformTreeContext, view_kind::SpatialViewKind};

use super::{SpatialViewVisualizerData, filter_visualizable_3d_entities};

pub struct TransformArrows3DVisualizer(SpatialViewVisualizerData);

impl Default for TransformArrows3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

impl IdentifiedViewSystem for TransformArrows3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformArrows3D".into()
    }
}

struct Transform3DVisualizabilityFilter {
    visualizability_trigger_components: IntSet<ComponentType>,
}

impl re_viewer_context::DataBasedVisualizabilityFilter for Transform3DVisualizabilityFilter {
    fn update_visualizability(&mut self, event: &re_chunk_store::ChunkStoreEvent) -> bool {
        true
    }
}

impl VisualizerSystem for TransformArrows3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<TransformArrows3D>();
        query_info.required = Default::default();
        query_info
    }

    // TODO: Add `InstancePoses3D`
    fn data_based_visualizability_filter(
        &self,
    ) -> Option<Box<dyn re_viewer_context::DataBasedVisualizabilityFilter>> {
        Some(Box::new(Transform3DVisualizabilityFilter {
            visualizability_trigger_components: Transform3D::all_components()
                .iter()
                .chain(std::iter::once(&Points3D::descriptor_positions()))
                .filter_map(|descr| descr.component_type)
                .collect(),
        }))
    }

    fn filter_visualizable_entities(
        &self,
        entities: MaybeVisualizableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let transforms = context_systems.get::<TransformTreeContext>()?;

        let latest_at_query = re_chunk_store::LatestAtQuery::new(query.timeline, query.latest_at);

        // Counting all transforms ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            // Use transform without potential pinhole, since we don't want to visualize image-space coordinates.
            let Some(transform_info) =
                transforms.transform_info_for_entity(data_result.entity_path.hash())
            else {
                continue;
            };
            let world_from_obj = if let Some(pinhole_tree_root_info) =
                transforms.pinhole_tree_root_info(transform_info.tree_root())
            {
                if transform_info.tree_root()
                    == re_tf::TransformFrameIdHash::from_entity_path(&data_result.entity_path)
                {
                    // We're _at_ that pinhole.
                    // Don't apply the from-2D transform, stick with the last known 3D.
                    pinhole_tree_root_info.parent_root_from_pinhole_root
                } else {
                    // We're inside a 2D space. But this is a 3D transform.
                    // Something is wrong here and this is not the right place to report it.
                    // Better just don't draw the axis!
                    continue;
                }
            } else {
                transform_info.single_transform_required_for_entity(
                    &data_result.entity_path,
                    Transform3D::name(),
                )
            }
            .as_affine3a();

            // Note, we use this interface instead of `data_result.latest_at_with_blueprint_resolved_data` to avoid querying
            // for a bunch of unused components. The actual transform data comes out of the context manager and can't be
            // overridden via blueprint anyways.
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_at_query,
                data_result,
                [TransformArrows3D::descriptor_axis_length().component],
                false,
            );

            let axis_length: f32 = results
                .get_mono_with_fallback::<AxisLength>(
                    TransformArrows3D::descriptor_axis_length().component,
                )
                .into();

            if axis_length == 0.0 {
                // Don't draw axis and don't add to the bounding box!
                continue;
            }

            // Only add the center to the bounding box - the lines may be dependent on the bounding box, causing a feedback loop otherwise.
            self.0.add_bounding_box(
                data_result.entity_path.hash(),
                macaw::BoundingBox::ZERO,
                world_from_obj,
            );

            add_axis_arrows(
                ctx.tokens(),
                &mut line_builder,
                world_from_obj,
                Some(&data_result.entity_path),
                axis_length,
                query
                    .highlights
                    .entity_outline_mask(data_result.entity_path.hash())
                    .overall,
            );
        }

        Ok(vec![line_builder.into_draw_data()?.into()])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn add_axis_arrows(
    tokens: &re_ui::DesignTokens,
    line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
    world_from_obj: glam::Affine3A,
    ent_path: Option<&EntityPath>,
    axis_length: f32,
    outline_mask_ids: re_renderer::OutlineMaskPreference,
) {
    use re_renderer::renderer::LineStripFlags;

    // TODO(andreas): It would be nice if could display the ViewCoordinates axis names (left/right/up) as a tooltip on hover.

    let line_radius = re_renderer::Size::new_ui_points(1.0);

    let mut line_batch = line_builder
        .batch(ent_path.map_or("axis_arrows".to_owned(), |p| p.to_string()))
        .world_from_obj(world_from_obj)
        .triangle_cap_length_factor(10.0)
        .triangle_cap_width_factor(3.0)
        .outline_mask_ids(outline_mask_ids)
        .picking_object_id(re_renderer::PickingLayerObjectId(
            ent_path.map_or(0, |p| p.hash64()),
        ));
    let picking_instance_id = re_renderer::PickingLayerInstanceId(Instance::ALL.get());

    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::X * axis_length)
        .radius(line_radius)
        .color(tokens.axis_color_x)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::Y * axis_length)
        .radius(line_radius)
        .color(tokens.axis_color_y)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::Z * axis_length)
        .radius(line_radius)
        .color(tokens.axis_color_z)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
}
