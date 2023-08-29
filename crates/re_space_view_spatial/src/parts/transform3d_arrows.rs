use egui::Color32;
use re_log_types::EntityPath;
use re_renderer::LineStripSeriesBuilder;
use re_types::{
    components::{InstanceKey, Transform3D},
    Loggable as _,
};
use re_viewer_context::{
    ArchetypeDefinition, NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{SharedRenderBuilders, TransformContext},
    view_kind::SpatialSpaceViewKind,
};

use super::SpatialViewPartData;

pub struct Transform3DArrowsPart(SpatialViewPartData);

impl Default for Transform3DArrowsPart {
    fn default() -> Self {
        Self(SpatialViewPartData::new(Some(SpatialSpaceViewKind::ThreeD)))
    }
}

impl NamedViewSystem for Transform3DArrowsPart {
    fn name() -> re_viewer_context::ViewSystemName {
        "Transform3DArrows".into()
    }
}

impl ViewPartSystem for Transform3DArrowsPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Transform3D::name()]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut line_builder = view_ctx.get::<SharedRenderBuilders>()?.lines();
        let transforms = view_ctx.get::<TransformContext>()?;

        let store = &ctx.store_db.entity_db.data_store;
        let latest_at_query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);
        for (ent_path, props) in query.iter_entities_for_system(Self::name()) {
            if store
                .query_latest_component::<Transform3D>(ent_path, &latest_at_query)
                .is_none()
            {
                continue;
            }

            if !*props.transform_3d_visible {
                continue;
            }

            // Use transform without potential pinhole, since we don't want to visualize image-space coordinates.
            let Some(world_from_obj) = transforms.reference_from_entity_ignoring_pinhole(
                ent_path,
                store,
                &latest_at_query,
            ) else {
                continue;
            };

            // Only add the center to the bounding box - the lines may be dependent on the bounding box, causing a feedback loop otherwise.
            self.0
                .bounding_box
                .extend(world_from_obj.translation.into());

            // Given how simple transform gizmos are it would be nice to put them all into a single line batch.
            // However, we can set object picking ids only per batch.
            add_axis_arrows(
                &mut line_builder,
                world_from_obj,
                Some(ent_path),
                *props.transform_3d_size,
                query
                    .highlights
                    .entity_outline_mask(ent_path.hash())
                    .overall,
            );
        }

        Ok(Vec::new()) // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
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
    line_builder: &mut LineStripSeriesBuilder,
    world_from_obj: macaw::Affine3A,
    ent_path: Option<&EntityPath>,
    axis_length: f32,
    outline_mask_ids: re_renderer::OutlineMaskPreference,
) {
    use re_renderer::renderer::LineStripFlags;

    // TODO(andreas): It would be nice if could display the ViewCoordinates axis names (left/right/up) as a tooltip on hover.

    let line_radius = re_renderer::Size::new_points(1.0);

    let mut line_batch = line_builder
        .batch("axis_arrows")
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
}
