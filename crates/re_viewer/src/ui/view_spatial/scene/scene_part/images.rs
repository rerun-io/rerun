use std::sync::Arc;

use egui::NumExt;
use glam::Vec3;
use itertools::Itertools;

use re_arrow_store::LatestAtQuery;
use re_data_store::{query::visit_type_data_2, FieldName, InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Tensor, TensorTrait},
    msg_bundle::Component,
    IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::{caches::AsDynamicImage, ViewerContext},
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{instance_hash_if_interactive, paint_properties, AnyTensor},
            Image, SceneSpatial,
        },
        Annotations, DefaultColor,
    },
};

use super::ScenePart;

pub struct ImagesPartClassic;

fn push_tensor_texture<T: AsDynamicImage>(
    scene: &mut SceneSpatial,
    ctx: &mut ViewerContext<'_>,
    annotations: &Arc<Annotations>,
    world_from_obj: glam::Mat4,
    instance_hash: InstanceIdHash,
    tensor: &T,
    tint: egui::Rgba,
) {
    let tensor_view =
        ctx.cache
            .image
            .get_view_with_annotations(tensor, annotations, ctx.render_ctx);

    if let Some(texture_handle) = tensor_view.texture_handle {
        let (h, w) = (tensor.shape()[0].size as f32, tensor.shape()[1].size as f32);
        scene
            .primitives
            .textured_rectangles
            .push(re_renderer::renderer::TexturedRect {
                top_left_corner_position: world_from_obj.transform_point3(glam::Vec3::ZERO),
                extent_u: world_from_obj.transform_vector3(glam::Vec3::X * w),
                extent_v: world_from_obj.transform_vector3(glam::Vec3::Y * h),
                texture: texture_handle,
                texture_filter_magnification: re_renderer::renderer::TextureFilterMag::Nearest,
                texture_filter_minification: re_renderer::renderer::TextureFilterMin::Linear,
                multiplicative_tint: tint,
                // Push to background. Mostly important for mouse picking order!
                depth_offset: -1,
            });
        scene.primitives.textured_rectangles_ids.push(instance_hash);
    }
}

impl ScenePart for ImagesPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("ImagesPartClassic");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Image])
        {
            scene.num_logged_2d_objects += 1;

            let properties = query.obj_props.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let highlighted_paths = ctx.hovered().is_path_selected(obj_path.hash());

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           tensor: &re_log_types::ClassicTensor,
                           color: Option<&[u8; 4]>,
                           meter: Option<&f32>| {
                if !tensor.is_shaped_like_an_image() {
                    return;
                }

                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let annotations = scene.annotation_map.find(obj_path);
                let paint_props = paint_properties(
                    annotations
                        .class_description(None)
                        .annotation_info()
                        .color(color, DefaultColor::OpaqueWhite),
                    None,
                );

                if highlighted_paths.is_index_selected(instance_hash.instance_index_hash) {
                    let rect =
                        glam::vec2(tensor.shape()[1].size as f32, tensor.shape()[0].size as f32);
                    scene
                        .primitives
                        .line_strips
                        .batch("image outlines")
                        .world_from_obj(world_from_obj)
                        .add_axis_aligned_rectangle_outline_2d(glam::Vec2::ZERO, rect)
                        .color(paint_props.fg_stroke.color)
                        .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));
                }

                push_tensor_texture(
                    scene,
                    ctx,
                    &annotations,
                    world_from_obj,
                    instance_hash,
                    tensor,
                    paint_props.fg_stroke.color.into(),
                );

                scene.ui.images.push(Image {
                    instance_hash,
                    tensor: AnyTensor::ClassicTensor(tensor.clone()),
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
                            return Some(previous_group);
                        }
                        rectangle_group.push(rect);
                    }
                    if !rectangle_group.is_empty() {
                        Some(rectangle_group.drain(..).collect())
                    } else {
                        None
                    }
                })
        };
        // Then, change opacity & transformation for planes within group except the base plane.
        for mut grouped_rects in rects_grouped_by_plane {
            let total_num_images = grouped_rects.len();
            for (idx, rect) in grouped_rects.iter_mut().enumerate() {
                // Set depth offset for correct order and avoid z fighting when there is a 3d camera.
                // Keep behind depth offset 0 for correct picking order.
                rect.depth_offset =
                    (idx as isize - total_num_images as isize) as re_renderer::DepthOffset;

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
pub(crate) struct ImagesPart;

impl ImagesPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        entity_view: &EntityView<Tensor>,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        properties: &ObjectProps,
        ent_path: &ObjPath,
        world_from_obj: glam::Mat4,
    ) -> Result<(), QueryError> {
        let highlighted_paths = ctx.hovered().is_path_selected(ent_path.hash());

        for (instance, tensor, color) in itertools::izip!(
            entity_view.iter_instances()?,
            entity_view.iter_primary()?,
            entity_view.iter_component::<ColorRGBA>()?
        ) {
            if let Some(tensor) = tensor {
                if !tensor.is_shaped_like_an_image() {
                    return Ok(());
                }

                let instance_hash = {
                    if properties.interactive {
                        InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                    } else {
                        InstanceIdHash::NONE
                    }
                };

                let annotations = scene.annotation_map.find(ent_path);

                let color = annotations.class_description(None).annotation_info().color(
                    color.map(|c| c.to_array()).as_ref(),
                    DefaultColor::OpaqueWhite,
                );

                let paint_props = paint_properties(color, None);

                if highlighted_paths.is_index_selected(instance_hash.instance_index_hash) {
                    let rect =
                        glam::vec2(tensor.shape()[1].size as f32, tensor.shape()[0].size as f32);
                    scene
                        .primitives
                        .line_strips
                        .batch("image outlines")
                        .world_from_obj(world_from_obj)
                        .add_axis_aligned_rectangle_outline_2d(glam::Vec2::ZERO, rect)
                        .color(paint_props.fg_stroke.color)
                        .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));
                }

                push_tensor_texture(
                    scene,
                    ctx,
                    &annotations,
                    world_from_obj,
                    instance_hash,
                    &tensor,
                    paint_props.fg_stroke.color.into(),
                );

                //TODO(john) add this component
                let meter: Option<&f32> = None;
                scene.ui.images.push(Image {
                    instance_hash,
                    tensor: AnyTensor::ArrowTensor(tensor),
                    meter: meter.copied(),
                    annotations,
                });
            }
        }

        Ok(())
    }
}

impl ScenePart for ImagesPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("ImagesPart");

        for ent_path in query.obj_paths {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            let properties = query.obj_props.get(ent_path);

            match query_entity_with_primary::<Tensor>(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                &[ColorRGBA::name()],
            )
            .and_then(|entity_view| {
                Self::process_entity_view(
                    &entity_view,
                    scene,
                    ctx,
                    &properties,
                    ent_path,
                    world_from_obj,
                )
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
