use glam::Mat4;
use re_data_store::{query::visit_type_data_4, FieldName, InstanceIdHash, ObjPath};
use re_log_types::{
    field_types::{ClassId, ColorRGBA, Rect2D},
    msg_bundle::Component,
    IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, QueryError};
use re_renderer::Size;

use crate::{
    misc::{ObjectPathSelectionScope, ViewerContext},
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{scene::instance_hash_if_interactive, Label2D, Label2DTarget, SceneSpatial},
        DefaultColor,
    },
};

use super::ScenePart;

/// `ScenePart` for classic data path
pub struct Boxes2DPartClassic;

impl ScenePart for Boxes2DPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("Boxes2DPartClassic");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::BBox2D])
        {
            scene.num_logged_2d_objects += 1;

            let properties = query.obj_props.get(obj_path);
            let annotations = scene.annotation_map.find(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let hovered_paths = ctx.hovered().check_obj_path(obj_path.hash());
            let selected_paths = ctx.selection().check_obj_path(obj_path.hash());

            let mut line_batch = scene
                .primitives
                .line_strips
                .batch("2d box")
                .world_from_obj(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           bbox: &re_log_types::BBox2D,
                           color: Option<&[u8; 4]>,
                           stroke_width: Option<&f32>,
                           label: Option<&String>,
                           class_id: Option<&i32>| {
                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let annotation_info = annotations
                    .class_description(class_id.map(|i| ClassId(*i as _)))
                    .annotation_info();
                let mut color = annotation_info.color(color, DefaultColor::ObjPath(obj_path));
                let mut radius = stroke_width.map_or(Size::AUTO, |r| Size::new_scene(r * 0.5));
                let label = annotation_info.label(label);

                SceneSpatial::apply_hover_and_selection_effect(
                    &mut radius,
                    &mut color,
                    instance_hash.instance_index_hash,
                    &hovered_paths,
                    &selected_paths,
                );

                line_batch
                    .add_axis_aligned_rectangle_outline_2d(bbox.min.into(), bbox.max.into())
                    .color(color)
                    .radius(radius)
                    .user_data(instance_hash);

                if let Some(label) = label {
                    scene.ui.labels_2d.push(Label2D {
                        text: label,
                        color,
                        target: Label2DTarget::Rect(egui::Rect::from_min_max(
                            bbox.min.into(),
                            bbox.max.into(),
                        )),
                        labled_instance: instance_hash,
                    });
                }
            };

            visit_type_data_4(
                obj_store,
                &FieldName::from("bbox"),
                &time_query,
                ("color", "stroke_width", "label", "class_id"),
                visitor,
            );
        }
    }
}

pub struct Boxes2DPart;

impl Boxes2DPart {
    /// Build scene parts for a single box instance
    #[allow(clippy::too_many_arguments)]
    fn visit_instance(
        scene: &mut SceneSpatial,
        obj_path: &ObjPath,
        world_from_obj: Mat4,
        instance: InstanceIdHash,
        rect: &Rect2D,
        color: Option<ColorRGBA>,
        hovered_paths: &ObjectPathSelectionScope,
        selected_paths: &ObjectPathSelectionScope,
    ) {
        scene.num_logged_2d_objects += 1;

        let color = color.map(|c| c.to_array());

        // TODO(jleibs): Lots of missing components
        let class_id = Some(&1);
        let label: Option<&String> = None;

        let annotations = scene.annotation_map.find(obj_path);
        let annotation_info = annotations
            .class_description(class_id.map(|i| ClassId(*i as _)))
            .annotation_info();
        let mut radius = Size::AUTO;
        let mut color = annotation_info.color(color.as_ref(), DefaultColor::ObjPath(obj_path));
        let label = annotation_info.label(label);

        SceneSpatial::apply_hover_and_selection_effect(
            &mut radius,
            &mut color,
            instance.instance_index_hash,
            hovered_paths,
            selected_paths,
        );

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("2d box")
            .world_from_obj(world_from_obj);

        line_batch
            .add_rectangle_outline_2d(
                glam::vec2(rect.x, rect.y),
                glam::vec2(rect.w, 0.0),
                glam::vec2(0.0, rect.h),
            )
            .color(color)
            .radius(radius)
            .user_data(instance);

        if let Some(label) = label {
            scene.ui.labels_2d.push(Label2D {
                text: label,
                color,
                target: Label2DTarget::Rect(egui::Rect::from_min_size(
                    egui::pos2(rect.x, rect.y),
                    egui::vec2(rect.w, rect.h),
                )),
                labled_instance: instance,
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
    ) {
        crate::profile_scope!("Boxes2DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Rect2D>(
                &ctx.log_db.obj_db.arrow_store,
                &query,
                ent_path,
                &[ColorRGBA::name()],
            )
            .and_then(|entity_view| {
                let hovered_paths = ctx.hovered().check_obj_path(ent_path.hash());
                let selected_paths = ctx.selection().check_obj_path(ent_path.hash());

                entity_view.visit2(|instance, rect, color| {
                    let instance_hash = {
                        if props.interactive {
                            InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                        } else {
                            InstanceIdHash::NONE
                        }
                    };

                    Self::visit_instance(
                        scene,
                        ent_path,
                        world_from_obj,
                        instance_hash,
                        &rect,
                        color,
                        &hovered_paths,
                        &selected_paths,
                    );
                })
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
