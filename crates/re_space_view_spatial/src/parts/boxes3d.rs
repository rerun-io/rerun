use re_components::{
    Box3D, ClassId, ColorRGBA, Component as _, InstanceKey, Label, Quaternion, Radius, Vec3D,
};
use re_data_store::EntityPath;
use re_query::{EntityView, QueryError};
use re_renderer::Size;
use re_viewer_context::{
    ArchetypeDefinition, DefaultColor, SpaceViewHighlights, ViewPartSystem, ViewQuery,
    ViewerContext,
};

use crate::{
    contexts::{SpatialSceneEntityContext, SpatialViewContext},
    parts::{entity_iterator::process_entity_views, UiLabel, UiLabelTarget},
    SpatialSpaceView,
};

use super::{picking_id_from_instance_key, SpatialSpaceViewState, SpatialViewPartData};

#[derive(Default)]
pub struct Boxes3DPart(SpatialViewPartData);

impl Boxes3DPart {
    fn process_entity_view(
        &mut self,
        _query: &ViewQuery<'_>,
        ent_view: &EntityView<Box3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let default_color = DefaultColor::EntityPath(ent_path);

        let mut line_builder = ent_context.shared_render_builders.lines();
        let mut line_batch = line_builder
            .batch("box 3d")
            .world_from_obj(ent_context.world_from_obj)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let visitor = |instance_key: InstanceKey,
                       half_size: Box3D,
                       position: Option<Vec3D>,
                       rotation: Option<Quaternion>,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       label: Option<Label>,
                       class_id: Option<ClassId>| {
            let class_description = ent_context.annotations.class_description(class_id);
            let annotation_info = class_description.annotation_info();

            let radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            let half_size = glam::Vec3::from(half_size);
            let rot = rotation.map(glam::Quat::from).unwrap_or_default();
            let tran = position.map_or(glam::Vec3::ZERO, glam::Vec3::from);
            let transform =
                glam::Affine3A::from_scale_rotation_translation(half_size * 2.0, rot, tran);

            let box_lines = line_batch
                .add_box_outline(transform)
                .radius(radius)
                .color(color)
                .picking_instance_id(picking_id_from_instance_key(instance_key));

            if let Some(outline_mask_ids) = ent_context.highlight.instances.get(&instance_key) {
                box_lines.outline_mask_ids(*outline_mask_ids);
            }

            if let Some(label) = annotation_info.label(label.as_ref().map(|s| &s.0)) {
                self.0.ui_labels.push(UiLabel {
                    text: label,
                    target: UiLabelTarget::Position3D(
                        ent_context.world_from_obj.transform_point3(tran),
                    ),
                    color,
                    labeled_instance: re_data_store::InstancePathHash::instance(
                        ent_path,
                        instance_key,
                    ),
                });
            }

            self.0.extend_bounding_box(
                // Good enough for now.
                macaw::BoundingBox::from_center_size(
                    tran,
                    glam::Vec3::splat(half_size.max_element()),
                ),
                ent_context.world_from_obj,
            );
        };

        ent_view.visit7(visitor)
    }
}

impl ViewPartSystem<SpatialSpaceView> for Boxes3DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![
            Box3D::name(),
            InstanceKey::name(),
            Vec3D::name(),      // obb.position
            Quaternion::name(), // obb.rotation
            ColorRGBA::name(),
            Radius::name(), // stroke_width
            Label::name(),
            ClassId::name(),
        ]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _space_view_state: &SpatialSpaceViewState,
        context: &SpatialViewContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("Boxes3DPart");

        process_entity_views::<Box3D, 8, _>(
            ctx,
            query,
            context,
            highlights,
            context.depth_offsets.points,
            self.archetype(),
            |_ctx, ent_path, entity_view, ent_context| {
                context
                    .num_3d_primitives
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.process_entity_view(query, &entity_view, ent_path, ent_context)
            },
        );

        Vec::new() // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&SpatialViewPartData> {
        Some(&self.0)
    }
}
