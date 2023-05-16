use re_data_store::EntityPath;
use re_log_types::{
    component_types::{Box3D, ClassId, ColorRGBA, InstanceKey, Label, Quaternion, Radius, Vec3D},
    Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::Size;
use re_viewer_context::{DefaultColor, SceneQuery, ViewerContext};

use crate::{
    misc::{SpaceViewHighlights, SpaceViewOutlineMasks, TransformCache},
    ui::view_spatial::{scene::EntityDepthOffsets, SceneSpatial, UiLabel, UiLabelTarget},
};

use super::{instance_key_to_picking_id, instance_path_hash_for_picking, ScenePart};

pub struct Boxes3DPart;

impl Boxes3DPart {
    fn process_entity_view(
        scene: &mut SceneSpatial,
        entity_view: &EntityView<Box3D>,
        ent_path: &EntityPath,
        world_from_obj: glam::Affine3A,
        entity_highlight: &SpaceViewOutlineMasks,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);
        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("box 3d")
            .world_from_obj(world_from_obj)
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        let visitor = |instance_key: InstanceKey,
                       half_size: Box3D,
                       position: Option<Vec3D>,
                       rotation: Option<Quaternion>,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       label: Option<Label>,
                       class_id: Option<ClassId>| {
            let class_description = annotations.class_description(class_id);
            let annotation_info = class_description.annotation_info();

            let radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);

            let scale = glam::Vec3::from(half_size) * 2.0;
            let rot = rotation.map(glam::Quat::from).unwrap_or_default();
            let tran = position.map_or(glam::Vec3::ZERO, glam::Vec3::from);
            let transform = glam::Affine3A::from_scale_rotation_translation(scale, rot, tran);

            let box_lines = line_batch
                .add_box_outline(transform)
                .radius(radius)
                .color(color)
                .picking_instance_id(instance_key_to_picking_id(
                    instance_key,
                    entity_view.num_instances(),
                    entity_highlight.any_selection_highlight,
                ));

            if let Some(outline_mask_ids) = entity_highlight.instances.get(&instance_key) {
                box_lines.outline_mask_ids(*outline_mask_ids);
            }

            if let Some(label) = annotation_info.label(label.as_ref().map(|s| &s.0)) {
                scene.ui.labels.push(UiLabel {
                    text: label,
                    target: UiLabelTarget::Position3D(world_from_obj.transform_point3(tran)),
                    color,
                    labeled_instance: instance_path_hash_for_picking(
                        ent_path,
                        instance_key,
                        entity_view.num_instances(),
                        entity_highlight.any_selection_highlight,
                    ),
                });
            }
        };

        entity_view.visit7(visitor)
    }
}

impl ScenePart for Boxes3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
        _depth_offsets: &EntityDepthOffsets,
    ) {
        crate::profile_scope!("Boxes3DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };
            let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

            match query_primary_with_history::<Box3D, 8>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    Box3D::name(),
                    InstanceKey::name(),
                    Vec3D::name(),      // obb.position
                    Quaternion::name(), // obb.rotation
                    ColorRGBA::name(),
                    Radius::name(), // stroke_width
                    Label::name(),
                    ClassId::name(),
                ],
            )
            .and_then(|entities| {
                for entity in entities {
                    Self::process_entity_view(
                        scene,
                        &entity,
                        ent_path,
                        world_from_obj,
                        entity_highlight,
                    )?;
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                }
            }
        }
    }
}
