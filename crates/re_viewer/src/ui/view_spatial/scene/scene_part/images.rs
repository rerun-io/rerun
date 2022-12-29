use egui::NumExt;
use glam::Vec3;
use itertools::Itertools;
use re_data_store::{query::visit_type_data_2, FieldName, InstanceIdHash, ObjectsProperties};
use re_log_types::{IndexHash, MsgId, ObjectType};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{instance_hash_if_interactive, paint_properties},
            Image, SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct ImagesPart;

impl ScenePart for ImagesPart {
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
            query.iter_object_stores(ctx.log_db, &[ObjectType::Image])
        {
            let properties = objects_properties.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           tensor: &re_log_types::Tensor,
                           color: Option<&[u8; 4]>,
                           meter: Option<&f32>| {
                if !tensor.is_shaped_like_an_image() {
                    return;
                }

                let (h, w) = (tensor.shape[0].size as f32, tensor.shape[1].size as f32);

                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let annotations = scene.annotation_map.find(obj_path);
                let color = annotations
                    .class_description(None)
                    .annotation_info()
                    .color(color, DefaultColor::OpaqueWhite);

                let paint_props = paint_properties(color, None);

                if instance_hash.is_some() && hovered_instance == instance_hash {
                    scene
                        .primitives
                        .line_strips
                        .batch("image outlines")
                        .add_axis_aligned_rectangle_outline_2d(glam::Vec2::ZERO, glam::vec2(w, h))
                        .color(paint_props.fg_stroke.color)
                        .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));
                }

                let legend = Some(annotations.clone());
                let tensor_view =
                    ctx.cache
                        .image
                        .get_view_with_annotations(tensor, &legend, ctx.render_ctx);

                if let Some(texture_handle) = tensor_view.texture_handle {
                    scene.primitives.textured_rectangles.push(
                        re_renderer::renderer::TexturedRect {
                            top_left_corner_position: world_from_obj
                                .transform_point3(glam::Vec3::ZERO),
                            extent_u: world_from_obj.transform_vector3(glam::Vec3::X * w),
                            extent_v: world_from_obj.transform_vector3(glam::Vec3::Y * h),
                            texture: texture_handle,
                            texture_filter_magnification:
                                re_renderer::renderer::TextureFilterMag::Nearest,
                            texture_filter_minification:
                                re_renderer::renderer::TextureFilterMin::Linear,
                            multiplicative_tint: paint_props.fg_stroke.color.into(),
                        },
                    );
                    // TODO: put into ui data instead?
                    scene.primitives.textured_rectangles_ids.push(instance_hash);
                }

                scene.ui.images.push(Image {
                    instance_hash,
                    tensor: tensor.clone(),
                    meter: meter.copied(),
                    annotations,
                });
            };
            visit_type_data_2(
                obj_store,
                &FieldName::from("tensor"),
                &time_query,
                ("color", "meter"),
                visitor,
            );
        }

        // Handle layered rectangles that are on (roughly) the same plane and were logged in sequence.
        // First, group by similar plane.
        // TODO(andreas): Need planes later for picking as well!
        let rects_grouped_by_plane = {
            let mut cur_plane = macaw::Plane3::from_normal_dist(Vec3::NAN, std::f32::NAN);
            let mut rectangle_group = Vec::new();
            scene
                .primitives
                .textured_rectangles
                .iter_mut()
                .batching(move |it| {
                    for rect in it.by_ref() {
                        let prev_plane = cur_plane;
                        cur_plane = macaw::Plane3::from_normal_point(
                            rect.extent_u.cross(rect.extent_v).normalize(),
                            rect.top_left_corner_position,
                        );

                        // Are the image planes too unsimilar? Then this is a new group.
                        if !rectangle_group.is_empty()
                            && prev_plane.normal.dot(cur_plane.normal) < 0.99
                            && (prev_plane.d - cur_plane.d) < 0.01
                        {
                            let previous_group =
                                std::mem::replace(&mut rectangle_group, vec![rect]);
                            return Some((cur_plane, previous_group));
                        }
                        rectangle_group.push(rect);
                    }
                    if !rectangle_group.is_empty() {
                        Some((cur_plane, rectangle_group.drain(..).collect()))
                    } else {
                        None
                    }
                })
        };
        // Then, change opacity & transformation for planes within group except the base plane.
        for (plane, mut grouped_rects) in rects_grouped_by_plane {
            let total_num_images = grouped_rects.len();
            for (idx, rect) in grouped_rects.iter_mut().enumerate() {
                // Move a bit to avoid z fighting.
                rect.top_left_corner_position +=
                    plane.normal * (total_num_images - idx - 1) as f32 * 0.1;
                // make top images transparent
                let opacity = if idx == 0 {
                    1.0
                } else {
                    1.0 / total_num_images.at_most(20) as f32
                }; // avoid precision problems in framebuffer
                rect.multiplicative_tint = rect.multiplicative_tint.multiply(opacity);
            }
        }
    }
}
