use glam::Mat4;
use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::{
    component_types::{ClassId, ColorRGBA, InstanceKey, Label, Radius, Rect2D},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, QueryError};
use re_renderer::Size;

use crate::{
    misc::{OptionalSpaceViewEntityHighlight, SpaceViewHighlights, TransformCache, ViewerContext},
    ui::{
        scene::SceneQuery,
        view_spatial::{
            scene::scene_part::instance_path_hash_for_picking, Label2D, Label2DTarget, SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;
pub struct Boxes2DPart;

impl Boxes2DPart {
    /// Build scene parts for a single box instance
    #[allow(clippy::too_many_arguments)]
    fn visit_instance(
        scene: &mut SceneSpatial,
        entity_path: &EntityPath,
        world_from_obj: Mat4,
        instance_path_hash: InstancePathHash,
        rect: &Rect2D,
        color: Option<ColorRGBA>,
        radius: Option<Radius>,
        label: Option<Label>,
        class_id: Option<ClassId>,
        entity_highlight: OptionalSpaceViewEntityHighlight<'_>,
    ) {
        scene.num_logged_2d_objects += 1;

        let annotations = scene.annotation_map.find(entity_path);
        let annotation_info = annotations.class_description(class_id).annotation_info();
        let mut color = annotation_info.color(
            color.map(|c| c.to_array()).as_ref(),
            DefaultColor::EntityPath(entity_path),
        );
        let mut radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
        let label = annotation_info.label(label.map(|l| l.0).as_ref());

        SceneSpatial::apply_hover_and_selection_effect(
            &mut radius,
            &mut color,
            entity_highlight.index_highlight(instance_path_hash.instance_key),
        );

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("2d box")
            .world_from_obj(world_from_obj);

        line_batch
            .add_rectangle_outline_2d(
                rect.top_left_corner().into(),
                glam::vec2(rect.width(), 0.0),
                glam::vec2(0.0, rect.height()),
            )
            .color(color)
            .radius(radius)
            .user_data(instance_path_hash);

        if let Some(label) = label {
            scene.ui.labels_2d.push(Label2D {
                text: label,
                color,
                target: Label2DTarget::Rect(egui::Rect::from_min_size(
                    rect.top_left_corner().into(),
                    egui::vec2(rect.width(), rect.height()),
                )),
                labled_instance: instance_path_hash,
            });
        }
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
    ) {
        crate::profile_scope!("Boxes2DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };

            let entity_highlight = highlights.entity_highlight(ent_path.hash());

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
                    entity_view.visit5(|instance_key, rect, color, radius, label, class_id| {
                        let instance_hash = instance_path_hash_for_picking(
                            ent_path,
                            instance_key,
                            &entity_view,
                            &props,
                            entity_highlight,
                        );
                        Self::visit_instance(
                            scene,
                            ent_path,
                            world_from_obj,
                            instance_hash,
                            &rect,
                            color,
                            radius,
                            label,
                            class_id,
                            entity_highlight,
                        );
                    })?;
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
