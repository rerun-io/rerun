use egui::NumExt;
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
            let ReferenceFromObjTransform::Rigid(world_from_obj) = transforms.reference_from_obj(obj_path) else {
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

        let total_num_images = scene.primitives.textured_rectangles.len();
        for (image_idx, img) in scene.primitives.textured_rectangles.iter_mut().enumerate() {
            img.top_left_corner_position = glam::vec3(
                0.0,
                0.0,
                // We use RDF (X=Right, Y=Down, Z=Forward) for 2D spaces, so we want lower Z in order to put images on top
                (total_num_images - image_idx - 1) as f32 * 0.1,
            );

            let opacity = if image_idx == 0 {
                1.0 // bottom image
            } else {
                // make top images transparent
                1.0 / total_num_images.at_most(20) as f32 // avoid precision problems in framebuffer
            };
            img.multiplicative_tint = img.multiplicative_tint.multiply(opacity);
        }
    }
}
