use egui::Color32;
use re_log_types::EntityPath;
use re_types::components::{InstanceKey, Transform3D};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    contexts::TransformContext, view_kind::SpatialSpaceViewKind,
    visualizers::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};

pub struct Transform3DArrowsVisualizer(SpatialViewVisualizerData);

impl Default for Transform3DArrowsVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

impl IdentifiedViewSystem for Transform3DArrowsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Transform3DArrows".into()
    }
}

impl VisualizerSystem for Transform3DArrowsVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<re_types::archetypes::Transform3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let transforms = view_ctx.get::<TransformContext>()?;

        let store = ctx.entity_db.store();
        let latest_at_query = re_data_store::LatestAtQuery::new(query.timeline, query.latest_at);

        // Counting all transform ahead of time is a bit wasteful and we don't expect a huge amount of lines from them,
        // so use the `LineDrawableBuilderAllocator` utility!
        const LINES_PER_BATCH_BUILDER: u32 = 3 * 32; // 32 transforms per line builder (each transform draws 3 lines)
        let mut line_builder = re_renderer::LineDrawableBuilderAllocator::new(
            ctx.render_ctx,
            LINES_PER_BATCH_BUILDER,
            LINES_PER_BATCH_BUILDER * 2, // Strips with 2 vertices each.
            SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            if store
                .query_latest_component::<Transform3D>(&data_result.entity_path, &latest_at_query)
                .is_none()
            {
                continue;
            }

            if !*data_result.accumulated_properties().transform_3d_visible {
                continue;
            }

            // Use transform without potential pinhole, since we don't want to visualize image-space coordinates.
            let Some(world_from_obj) = transforms.reference_from_entity_ignoring_pinhole(
                &data_result.entity_path,
                store,
                &latest_at_query,
            ) else {
                continue;
            };

            // Only add the center to the bounding box - the lines may be dependent on the bounding box, causing a feedback loop otherwise.
            self.0.add_bounding_box(
                data_result.entity_path.hash(),
                macaw::BoundingBox::ZERO,
                world_from_obj,
            );

            add_axis_arrows(
                &mut line_builder,
                world_from_obj,
                Some(&data_result.entity_path),
                *data_result.accumulated_properties().transform_3d_size,
                query
                    .highlights
                    .entity_outline_mask(data_result.entity_path.hash())
                    .overall,
            )?;
        }

        Ok(line_builder.finish()?)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

pub fn add_axis_arrows(
    line_builder: &mut re_renderer::LineDrawableBuilderAllocator<'_>,
    world_from_obj: macaw::Affine3A,
    ent_path: Option<&EntityPath>,
    axis_length: f32,
    outline_mask_ids: re_renderer::OutlineMaskPreference,
) -> Result<(), re_renderer::renderer::LineDrawDataError> {
    use re_renderer::renderer::LineStripFlags;

    // TODO(andreas): It would be nice if could display the ViewCoordinates axis names (left/right/up) as a tooltip on hover.

    let line_radius = re_renderer::Size::new_points(1.0);

    let batch_name = ent_path.map_or("axis_arrows".to_owned(), |p| p.to_string());
    let mut line_batch = line_builder
        .reserve_batch(batch_name, 3, 6)?
        .world_from_obj(world_from_obj)
        .triangle_cap_length_factor(10.0)
        .triangle_cap_width_factor(3.0)
        .outline_mask_ids(outline_mask_ids)
        .picking_object_id(re_renderer::PickingLayerObjectId(
            ent_path.map_or(0, |p| p.hash64()),
        ));
    let picking_instance_id = re_renderer::PickingLayerInstanceId(InstanceKey::SPLAT.0);

    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::X * axis_length)
        .radius(line_radius)
        .color(AXIS_COLOR_X)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::Y * axis_length)
        .radius(line_radius)
        .color(AXIS_COLOR_Y)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);
    line_batch
        .add_segment(glam::Vec3::ZERO, glam::Vec3::Z * axis_length)
        .radius(line_radius)
        .color(AXIS_COLOR_Z)
        .flags(LineStripFlags::FLAG_CAP_END_TRIANGLE | LineStripFlags::FLAG_CAP_START_ROUND)
        .picking_instance_id(picking_instance_id);

    Ok(())
}
