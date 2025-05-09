use egui::Color32;
use nohash_hasher::IntSet;
use re_log_types::{EntityPath, Instance};
use re_types::{
    archetypes::{Pinhole, Transform3D},
    components::{AxisLength, ImagePlaneDistance},
    Archetype as _, ComponentName,
};
use re_view::{latest_at_with_blueprint_resolved_data, DataResultQuery as _};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, QueryContext, TypedComponentFallbackProvider,
    ViewContext, ViewContextCollection, ViewQuery, ViewStateExt as _, ViewSystemExecutionError,
    VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{contexts::TransformTreeContext, ui::SpatialViewState, view_kind::SpatialViewKind};

use super::{filter_visualizable_3d_entities, CamerasVisualizer, SpatialViewVisualizerData};

pub struct Transform3DArrowsVisualizer(SpatialViewVisualizerData);

impl Default for Transform3DArrowsVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

impl IdentifiedViewSystem for Transform3DArrowsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Transform3DArrows".into()
    }
}

struct Transform3DVisualizabilityFilter {
    visualizability_trigger_components: IntSet<ComponentName>,
}

impl re_viewer_context::DataBasedVisualizabilityFilter for Transform3DVisualizabilityFilter {
    fn update_visualizability(&mut self, event: &re_chunk_store::ChunkStoreEvent) -> bool {
        // There's no required component on `Transform3D` archetype, so by default it would always be visualizable.
        // That's not entirely wrong, after all, the transform arrows make always sense!
        // But today, this notion messes with a lot of things:
        // * it means everything can be visualized in a 3D view!
        // * if there's no indicated visualizer, we show any visualizer that is visualizable (that would be this one always then)
        event.diff.chunk.component_names().any(|component_name| {
            self.visualizability_trigger_components
                .contains(&component_name)
        })
    }
}

impl VisualizerSystem for Transform3DArrowsVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Transform3D>()
    }

    fn data_based_visualizability_filter(
        &self,
    ) -> Option<Box<dyn re_viewer_context::DataBasedVisualizabilityFilter>> {
        Some(Box::new(Transform3DVisualizabilityFilter {
            visualizability_trigger_components: Transform3D::all_components()
                .iter()
                .map(|descr| descr.component_name)
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
            let world_from_obj = if let Some(twod_in_threed_info) =
                &transform_info.twod_in_threed_info
            {
                if twod_in_threed_info.parent_pinhole != data_result.entity_path {
                    // We're inside a 2D space. But this is a 3D transform.
                    // Something is wrong here and this is not the right place to report it.
                    // Better just don't draw the axis!
                    continue;
                } else {
                    // Don't apply the from-2D transform, stick with the last known 3D.
                    twod_in_threed_info.reference_from_pinhole_entity
                }
            } else {
                transform_info
                    .single_entity_transform_required(&data_result.entity_path, "Transform3DArrows")
            };

            // Note, we use this interface instead of `data_result.latest_at_with_blueprint_resolved_data` to avoid querying
            // for a bunch of unused components. The actual transform data comes out of the context manager and can't be
            // overridden via blueprint anyways.
            let results = latest_at_with_blueprint_resolved_data(
                ctx,
                None,
                &latest_at_query,
                data_result,
                [&Transform3D::descriptor_axis_length()],
                false,
            );

            let axis_length: f32 = results
                .get_mono_with_fallback::<AxisLength>(&Transform3D::descriptor_axis_length())
                .into();

            if axis_length == 0.0 {
                // Don't draw axis and don't add to the bounding box!
                continue;
            }

            // Only add the center to the bounding box - the lines may be dependent on the bounding box, causing a feedback loop otherwise.
            self.0.add_bounding_box(
                data_result.entity_path.hash(),
                re_math::BoundingBox::ZERO,
                world_from_obj,
            );

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

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

pub fn add_axis_arrows(
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
        if let Some(view_ctx) = ctx.view_ctx {
            let query_result = ctx.viewer_ctx.lookup_query_result(view_ctx.view_id);

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
                        let results = data_result
                            .latest_at_with_blueprint_resolved_data::<Pinhole>(view_ctx, ctx.query);

                        Some(results.get_mono_with_fallback::<ImagePlaneDistance>(
                            &Pinhole::descriptor_image_plane_distance(),
                        ))
                    } else {
                        None
                    }
                })
            {
                let length: f32 = length.into();
                return (length * 0.5).into();
            }
        }

        // If there is a finite bounding box, use the scene size to determine the axis length.
        if let Ok(state) = ctx.view_state.downcast_ref::<SpatialViewState>() {
            let scene_size = state.bounding_boxes.smoothed.size().length();

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

/// The `AxisLengthDetector` doesn't actually visualize anything, but it allows us to detect
/// when a transform has set the [`AxisLength`] component.
///
/// See the logic in [`crate::SpatialView3D`]`::choose_default_visualizers`.
#[derive(Default)]
pub struct AxisLengthDetector();

impl IdentifiedViewSystem for AxisLengthDetector {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "AxisLengthDetector".into()
    }
}

impl VisualizerSystem for AxisLengthDetector {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<Transform3D>();
        query_info.indicators = Default::default();

        query_info
            .required
            .insert(Transform3D::descriptor_axis_length());

        query_info
    }

    fn execute(
        &mut self,
        _ctx: &ViewContext<'_>,
        _query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        Ok(vec![])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }

    #[inline]
    fn filter_visualizable_entities(
        &self,
        _entities: MaybeVisualizableEntities,
        _context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        // Never actually visualize this detector
        Default::default()
    }
}

re_viewer_context::impl_component_fallback_provider!(AxisLengthDetector => []);
