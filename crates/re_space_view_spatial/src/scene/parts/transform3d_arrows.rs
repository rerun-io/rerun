use egui::Color32;
use re_components::{Component as _, Transform3D};
use re_log_types::EntityPath;
use re_renderer::LineStripSeriesBuilder;
use re_viewer_context::{
    ArchetypeDefinition, ScenePart, SceneQuery, SpaceViewHighlights, ViewerContext,
};

use crate::{scene::contexts::SpatialSceneContext, SpatialSpaceView};

use super::{SpatialScenePartData, SpatialSpaceViewState};

#[derive(Default)]
pub struct Transform3DArrowsPart(SpatialScenePartData);

impl ScenePart<SpatialSpaceView> for Transform3DArrowsPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Transform3D::name()]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        space_view_state: &SpatialSpaceViewState,
        scene_context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("TransformGizmoPart");

        let mut line_builder = scene_context.shared_render_builders.lines();

        // Origin gizmo if requested.
        // TODO(#2522): This is incompatible with the refactor in #2522 which no longer allows access to the space_view_state.
        //              Does this need to move to a context?
        if space_view_state.state_3d.show_axes {
            let axis_length = 1.0; // The axes are also a measuring stick
            add_axis_lines(
                &mut line_builder,
                macaw::Affine3A::IDENTITY,
                None,
                axis_length,
                re_renderer::OutlineMaskPreference::NONE,
            );
        }

        let store = &ctx.store_db.entity_db.data_store;
        let latest_at_query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);
        for (ent_path, props) in query.iter_entities() {
            if store
                .query_latest_component::<Transform3D>(ent_path, &latest_at_query)
                .is_none()
            {
                continue;
            }

            scene_context
                .num_3d_primitives
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            if !props.transform_3d_visible.get() {
                continue;
            }

            // Apply the transform _and_ the parent transform, but if we're at a pinhole camera ignore that part.
            let Some(world_from_obj) = scene_context.transforms.
                reference_from_entity_ignore_pinhole(ent_path, store, &latest_at_query) else {
                continue;
            };

            // Only add the center to the bounding box - the lines may be dependent on the bounding box, causing a feedback loop otherwise.
            self.0
                .bounding_box
                .extend(world_from_obj.translation.into());

            // Given how simple transform gizmos are it would be nice to put them all into a single line batch.
            // However, we can set object picking ids only per batch.
            add_axis_lines(
                &mut line_builder,
                world_from_obj,
                Some(ent_path),
                *props.transform_3d_size.get(),
                highlights.entity_outline_mask(ent_path.hash()).overall,
            );
        }

        Vec::new() // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&SpatialScenePartData> {
        Some(&self.0)
    }
}

const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

pub fn add_axis_lines(
    line_builder: &mut LineStripSeriesBuilder,
    world_from_obj: macaw::Affine3A,
    ent_path: Option<&EntityPath>,
    axis_length: f32,
    outline_mask_ids: re_renderer::OutlineMaskPreference,
) {
    use re_renderer::renderer::LineStripFlags;

    // TODO(andreas): It would be nice if could display the semantics (left/right/up) as a tooltip on hover.

    let line_radius = re_renderer::Size::new_points(1.0);

    let mut line_batch = line_builder
        .batch("transform gizmo")
        .world_from_obj(world_from_obj)
        .triangle_cap_length_factor(10.0)
        .triangle_cap_width_factor(3.0)
        .outline_mask_ids(outline_mask_ids)
        .picking_object_id(re_renderer::PickingLayerObjectId(
            ent_path.map_or(0, |p| p.hash64()),
        ));
    let picking_instance_id =
        re_renderer::PickingLayerInstanceId(re_log_types::InstanceKey::SPLAT.0);

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
