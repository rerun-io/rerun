use re_data_store::EntityPath;
use re_log_types::{
    component_types::{ClassId, ColorRGBA, InstanceKey, Label, Radius, Rect2D},
    Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::Size;
use re_viewer_context::{DefaultColor, SceneQuery, ViewerContext};

use crate::{
    misc::{SpaceViewHighlights, TransformCache},
    ui::view_spatial::{
        scene::{scene_part::instance_path_hash_for_picking, EntityDepthOffsets},
        SceneSpatial, UiLabel, UiLabelTarget,
    },
};

use super::{instance_key_to_picking_id, ScenePart};

pub struct Boxes2DPart;

impl Boxes2DPart {
    fn process_entity_view(
        scene: &mut SceneSpatial,
        entity_view: &EntityView<Rect2D>,
        ent_path: &EntityPath,
        world_from_obj: glam::Affine3A,
        highlights: &SpaceViewHighlights,
        depth_offset: re_renderer::DepthOffset,
    ) -> Result<(), QueryError> {
        scene.num_logged_2d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::EntityPath(ent_path);

        let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("2d boxes")
            .depth_offset(depth_offset)
            .world_from_obj(world_from_obj)
            .outline_mask_ids(entity_highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

        entity_view.visit5(
            |instance_key,
             rect,
             color: Option<ColorRGBA>,
             radius: Option<Radius>,
             label: Option<Label>,
             class_id: Option<ClassId>| {
                let instance_hash = instance_path_hash_for_picking(
                    ent_path,
                    instance_key,
                    entity_view.num_instances(),
                    entity_highlight.any_selection_highlight,
                );

                let annotation_info = annotations.class_description(class_id).annotation_info();
                let color =
                    annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
                let radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
                let label = annotation_info.label(label.map(|l| l.0).as_ref());

                let rectangle = line_batch
                    .add_rectangle_outline_2d(
                        rect.top_left_corner().into(),
                        glam::vec2(rect.width(), 0.0),
                        glam::vec2(0.0, rect.height()),
                    )
                    .color(color)
                    .radius(radius)
                    .picking_instance_id(instance_key_to_picking_id(
                        instance_key,
                        entity_view.num_instances(),
                        entity_highlight.any_selection_highlight,
                    ));

                if let Some(outline_mask_ids) =
                    entity_highlight.instances.get(&instance_hash.instance_key)
                {
                    rectangle.outline_mask_ids(*outline_mask_ids);
                }

                if let Some(label) = label {
                    scene.ui.labels.push(UiLabel {
                        text: label,
                        color,
                        target: UiLabelTarget::Rect(egui::Rect::from_min_size(
                            rect.top_left_corner().into(),
                            egui::vec2(rect.width(), rect.height()),
                        )),
                        labeled_instance: instance_hash,
                    });
                }
            },
        )
    }
}

impl ScenePart for Boxes2DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
        depth_offsets: &EntityDepthOffsets,
    ) {
        crate::profile_scope!("Boxes2DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };

            match query_primary_with_history::<Rect2D, 6>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    Rect2D::name(),
                    InstanceKey::name(),
                    ColorRGBA::name(),
                    Radius::name(),
                    Label::name(),
                    ClassId::name(),
                ],
            )
            .and_then(|entities| {
                for entity_view in entities {
                    Self::process_entity_view(
                        scene,
                        &entity_view,
                        ent_path,
                        world_from_obj,
                        highlights,
                        depth_offsets.get(ent_path).unwrap_or(depth_offsets.box2d),
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
