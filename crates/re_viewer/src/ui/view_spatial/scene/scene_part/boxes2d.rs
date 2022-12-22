use re_arrow_store::TimeQuery;
use re_data_store::{query::visit_type_data_4, FieldName, InstanceIdHash, ObjectsProperties};
use re_log_types::{
    context::ClassId,
    field_types::{ColorRGBA, Instance, Rect2D},
    msg_bundle::Component,
    IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, QueryError};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{apply_hover_effect, instance_hash_if_interactive, paint_properties},
            Label2D, Label2DTarget, SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Boxes2DPart;

impl ScenePart for Boxes2DPart {
    fn load(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        objects_properties: &ObjectsProperties,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::BBox2D])
        {
            let properties = objects_properties.get(obj_path);
            let annotations = scene.annotation_map.find(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

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
                let color = annotation_info.color(color, DefaultColor::ObjPath(obj_path));
                let label = annotation_info.label(label);

                // Hovering with a rect.
                let rect = egui::Rect::from_min_max(bbox.min.into(), bbox.max.into());
                scene.ui.rects.push((rect, instance_hash));

                let mut paint_props = paint_properties(color, stroke_width);
                if instance_hash.is_some() && hovered_instance == instance_hash {
                    apply_hover_effect(&mut paint_props);
                }

                // Lines don't associated with instance (i.e. won't participate in hovering)
                line_batch
                    .add_axis_aligned_rectangle_outline_2d(bbox.min.into(), bbox.max.into())
                    .color(paint_props.bg_stroke.color)
                    .radius(Size::new_points(paint_props.bg_stroke.width * 0.5))
                    .user_data(instance_hash);
                line_batch
                    .add_axis_aligned_rectangle_outline_2d(bbox.min.into(), bbox.max.into())
                    .color(paint_props.fg_stroke.color)
                    .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));

                if let Some(label) = label {
                    scene.ui.labels_2d.push(Label2D {
                        text: label,
                        color: paint_props.fg_stroke.color,
                        target: Label2DTarget::Rect(rect),
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

        // Second pass for arrow-stored rectangles
        for obj_path in query.obj_paths {
            let ent_path = obj_path;
            let timeline_query = re_arrow_store::TimelineQuery::new(
                query.timeline,
                TimeQuery::LatestAt(query.latest_at.as_i64()),
            );

            match query_entity_with_primary(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                Rect2D::name(),
                &[ColorRGBA::name()],
            )
            .and_then(|entity_view| {
                entity_view.visit2(
                    |instance: Instance, rect: Rect2D, color: Option<ColorRGBA>| {
                        let instance_hash =
                            InstanceIdHash::from_path_and_arrow_instance(obj_path, &instance);

                        let color = color.map(|c| c.to_array());

                        // TODO(jleibs): Lots of missing components
                        let class_id = Some(&1);
                        let label: Option<&String> = None;
                        let stroke_width: Option<&f32> = None;

                        let annotations = scene.annotation_map.find(obj_path);
                        let annotation_info = annotations
                            .class_description(class_id.map(|i| ClassId(*i as _)))
                            .annotation_info();
                        let color =
                            annotation_info.color(color.as_ref(), DefaultColor::ObjPath(obj_path));
                        let label = annotation_info.label(label);

                        // Hovering with a rect.
                        let hover_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.x, rect.y),
                            egui::vec2(rect.w, rect.h),
                        );
                        scene.ui.rects.push((hover_rect, instance_hash));

                        let mut paint_props = paint_properties(color, stroke_width);
                        if hovered_instance == instance_hash {
                            apply_hover_effect(&mut paint_props);
                        }

                        // Lines don't associated with instance (i.e. won't participate in hovering)
                        let mut line_batch = scene.primitives.line_strips.batch("2d box");
                        line_batch
                            .add_rectangle_outline_2d(
                                glam::vec2(rect.x, rect.y),
                                glam::vec2(rect.w, 0.0),
                                glam::vec2(0.0, rect.h),
                            )
                            .color(paint_props.bg_stroke.color)
                            .radius(Size::new_points(paint_props.bg_stroke.width * 0.5));

                        line_batch
                            .add_rectangle_outline_2d(
                                glam::vec2(rect.x, rect.y),
                                glam::vec2(rect.w, 0.0),
                                glam::vec2(0.0, rect.h),
                            )
                            .color(paint_props.fg_stroke.color)
                            .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));

                        if let Some(label) = label {
                            scene.ui.labels_2d.push(Label2D {
                                text: label,
                                color: paint_props.fg_stroke.color,
                                target: Label2DTarget::Rect(hover_rect),
                                labled_instance: instance_hash,
                            });
                        }
                    },
                )
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", obj_path, err);
                }
            }
        }
    }
}
