use egui::Color32;
use re_log_types::{EntityPath, Instance};
use re_space_view::DataResultQuery;
use re_types::{
    archetypes::{self, Axes3D, Pinhole},
    components::{AxisLength, ImagePlaneDistance, Transform3D},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, QueryContext, SpaceViewStateExt,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::TransformContext, ui::SpatialSpaceViewState, view_kind::SpatialSpaceViewKind,
    visualizers::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

use super::{filter_visualizable_3d_entities, CamerasVisualizer, SpatialViewVisualizerData};

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
        let mut query_info = VisualizerQueryInfo::from_archetype::<archetypes::Transform3D>();
        let mut axes_queried: ComponentNameSet = Axes3D::all_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect::<ComponentNameSet>();
        query_info.queried.append(&mut axes_queried);
        query_info.indicators = std::iter::once(Axes3D::indicator().name()).collect();
        query_info
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
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let transforms = context_systems.get::<TransformContext>()?;

        let latest_at_query = re_data_store::LatestAtQuery::new(query.timeline, query.latest_at);

        // Counting all transforms ahead of time is a bit wasteful, but we also don't expect a huge amount,
        // so let re_renderer's allocator internally decide what buffer sizes to pick & grow them as we go.
        let mut line_builder = re_renderer::LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            if ctx
                .recording()
                .latest_at_component::<Transform3D>(&data_result.entity_path, &latest_at_query)
                .is_none()
            {
                continue;
            }

            // Use transform without potential pinhole, since we don't want to visualize image-space coordinates.
            let Some(world_from_obj) = transforms.reference_from_entity_ignoring_pinhole(
                &data_result.entity_path,
                ctx.recording(),
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

            let results = data_result.latest_at_with_overrides::<Axes3D>(ctx, &latest_at_query);
            let axis_length = results.get_mono_with_fallback::<AxisLength>().into();

            add_axis_arrows(
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

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

pub fn add_axis_arrows(
    line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
    world_from_obj: macaw::Affine3A,
    ent_path: Option<&EntityPath>,
    axis_length: f32,
    outline_mask_ids: re_renderer::OutlineMaskPreference,
) {
    use re_renderer::renderer::LineStripFlags;

    // TODO(andreas): It would be nice if could display the ViewCoordinates axis names (left/right/up) as a tooltip on hover.

    let line_radius = re_renderer::Size::new_points(1.0);

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
}

impl TypedComponentFallbackProvider<AxisLength> for Transform3DArrowsVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> AxisLength {
        let query_result = ctx.view_ctx.lookup_query_result(ctx.view_ctx.view_id);

        // If there is a camera in the scene and it has a pinhole, use the image plane distance to determine the axis length.
        if let Some(length) = query_result
            .tree
            .lookup_result_by_path(ctx.target_entity_path)
            .cloned()
            .and_then(|data_result| {
                if data_result
                    .visualizers
                    .contains(&CamerasVisualizer::identifier())
                {
                    let results =
                        data_result.latest_at_with_overrides::<Pinhole>(ctx.view_ctx, ctx.query);

                    Some(results.get_mono_with_fallback::<ImagePlaneDistance>())
                } else {
                    None
                }
            })
        {
            let length: f32 = length.into();
            return (length * 0.5).into();
        }

        // If there is a finite bounding box, use the scene size to determine the axis length.
        if let Ok(state) = ctx
            .view_ctx
            .view_state
            .downcast_ref::<SpatialSpaceViewState>()
        {
            let scene_size = state.bounding_boxes.accumulated.size().length();

            if scene_size.is_finite() && scene_size > 0.0 {
                return (scene_size * 0.05).into();
            };
        }

        // Otherwise 0.3 is a reasonable default.

        // This value somewhat arbitrary. In almost all cases where the scene has defined bounds
        // the heuristic will change it or it will be user edited. In the case of non-defined bounds
        // this value works better with the default camera setup.
        0.3.into()
    }
}

re_viewer_context::impl_component_fallback_provider!(Transform3DArrowsVisualizer => [AxisLength]);
