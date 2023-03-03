use std::sync::Arc;

use egui::NumExt;
use glam::Vec3;
use itertools::Itertools;

use re_data_store::{query_latest_single, EntityPath, EntityProperties, InstancePathHash};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Tensor, TensorData, TensorDataMeaning, TensorTrait},
    msg_bundle::Component,
    Transform,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{
    renderer::{DepthCloud, DepthCloudDepthData},
    Size,
};

use crate::{
    misc::{caches::AsDynamicImage, SpaceViewHighlights, TransformCache, ViewerContext},
    ui::{
        scene::SceneQuery,
        view_spatial::{scene::scene_part::instance_path_hash_for_picking, Image, SceneSpatial},
        Annotations, DefaultColor,
    },
};

use super::ScenePart;

fn push_tensor_texture<T: AsDynamicImage>(
    scene: &mut SceneSpatial,
    ctx: &mut ViewerContext<'_>,
    annotations: &Arc<Annotations>,
    world_from_obj: glam::Mat4,
    instance_path_hash: InstancePathHash,
    tensor: &T,
    tint: egui::Rgba,
) {
    crate::profile_function!();

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
        scene
            .primitives
            .textured_rectangles_ids
            .push(instance_path_hash);
    }
}

fn handle_image_layering(scene: &mut SceneSpatial) {
    crate::profile_function!();

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
                        let previous_group = std::mem::replace(&mut rectangle_group, vec![rect]);
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

pub(crate) struct ImagesPart;

impl ImagesPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        entity_view: &EntityView<Tensor>,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        transforms: &TransformCache,
        properties: &EntityProperties,
        ent_path: &EntityPath,
        world_from_obj: glam::Mat4,
        highlights: &SpaceViewHighlights,
    ) -> Result<(), QueryError> {
        crate::profile_function!();

        for (instance_key, tensor, color) in itertools::izip!(
            entity_view.iter_instance_keys()?,
            entity_view.iter_primary()?,
            entity_view.iter_component::<ColorRGBA>()?
        ) {
            crate::profile_scope!("loop_iter");
            if let Some(tensor) = tensor {
                if !tensor.is_shaped_like_an_image() {
                    return Ok(());
                }

                if tensor.meaning == TensorDataMeaning::Depth {
                    if let Some(pinhole_ent_path) = properties.backproject_pinhole_ent_path.as_ref()
                    {
                        // NOTE: we don't pass in `world_from_obj` because this corresponds to the
                        // transform of the projection plane, which is of no use to us here.
                        // What we want are the extrinsics of the depth camera!
                        Self::process_entity_view_as_depth_cloud(
                            scene,
                            ctx,
                            transforms,
                            properties,
                            &tensor,
                            pinhole_ent_path,
                        );
                        return Ok(());
                    };
                }

                Self::process_entity_view_as_image(
                    entity_view,
                    scene,
                    ctx,
                    properties,
                    ent_path,
                    world_from_obj,
                    highlights,
                    instance_key,
                    tensor,
                    color,
                );
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view_as_image(
        entity_view: &EntityView<Tensor>,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        properties: &EntityProperties,
        ent_path: &EntityPath,
        world_from_obj: glam::Mat4,
        highlights: &SpaceViewHighlights,
        instance_key: InstanceKey,
        tensor: Tensor,
        color: Option<ColorRGBA>,
    ) {
        crate::profile_function!();

        let entity_highlight = highlights.entity_highlight(ent_path.hash());

        let instance_path_hash = instance_path_hash_for_picking(
            ent_path,
            instance_key,
            entity_view,
            properties,
            entity_highlight,
        );

        let annotations = scene.annotation_map.find(ent_path);

        let color = annotations.class_description(None).annotation_info().color(
            color.map(|c| c.to_array()).as_ref(),
            DefaultColor::OpaqueWhite,
        );

        let highlight = entity_highlight.index_highlight(instance_path_hash.instance_key);
        if highlight.is_some() {
            let color = SceneSpatial::apply_hover_and_selection_effect_color(
                re_renderer::Color32::TRANSPARENT,
                highlight,
            );
            let rect = glam::vec2(tensor.shape()[1].size as f32, tensor.shape()[0].size as f32);
            scene
                .primitives
                .line_strips
                .batch("image outlines")
                .world_from_obj(world_from_obj)
                .add_axis_aligned_rectangle_outline_2d(glam::Vec2::ZERO, rect)
                .color(color)
                .radius(Size::new_points(1.0));
        }

        push_tensor_texture(
            scene,
            ctx,
            &annotations,
            world_from_obj,
            instance_path_hash,
            &tensor,
            color.into(),
        );

        // TODO(jleibs): Meter should really be its own component
        let meter = tensor.meter;

        scene.ui.images.push(Image {
            instance_path_hash,
            tensor,
            meter,
            annotations,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view_as_depth_cloud(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        transforms: &TransformCache,
        properties: &EntityProperties,
        tensor: &Tensor,
        pinhole_ent_path: &EntityPath,
    ) {
        crate::profile_function!();

        let Some(re_log_types::Transform::Pinhole(intrinsics)) = query_latest_single::<Transform>(
            &ctx.log_db.entity_db,
            pinhole_ent_path,
            &ctx.current_query(),
        ) else {
            re_log::warn_once!("Couldn't fetch pinhole intrinsics at {pinhole_ent_path:?}");
            return;
        };

        // TODO(cmc): getting to those extrinsics is no easy task :|
        let Some(extrinsics) = pinhole_ent_path
            .parent()
            .and_then(|ent_path| transforms.reference_from_entity(&ent_path)) else {
            re_log::warn_once!("Couldn't fetch pinhole extrinsics at {pinhole_ent_path:?}");
            return;
        };

        // TODO(cmc): automagically convert as needed for non-natively supported datatypes?
        let data = match &tensor.data {
            // NOTE: Shallow clone if feature `arrow` is enabled, full alloc + memcpy otherwise.
            TensorData::U16(data) => DepthCloudDepthData::U16(data.clone()),
            TensorData::F32(data) => DepthCloudDepthData::F32(data.clone()),
            _ => {
                let discriminant = std::mem::discriminant(&tensor.data);
                re_log::warn_once!(
                    "Tensor datatype is not supported for backprojection ({discriminant:?})"
                );
                return;
            }
        };

        let scale = *properties.backproject_scale.get();
        let radius_scale = *properties.backproject_radius_scale.get();

        let (h, w) = (tensor.shape()[0].size, tensor.shape()[1].size);
        let dimensions = glam::UVec2::new(w as _, h as _);

        let world_from_obj = extrinsics * glam::Mat4::from_scale(glam::Vec3::splat(scale));

        scene.primitives.depth_clouds.push(DepthCloud {
            depth_camera_extrinsics: world_from_obj,
            depth_camera_intrinsics: intrinsics.image_from_cam.into(),
            radius_scale,
            depth_dimensions: dimensions,
            depth_data: data,
        });
    }
}

impl ScenePart for ImagesPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_scope!("ImagesPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };

            match query_primary_with_history::<Tensor, 3>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [Tensor::name(), InstanceKey::name(), ColorRGBA::name()],
            )
            .and_then(|entities| {
                for entity in entities {
                    Self::process_entity_view(
                        &entity,
                        scene,
                        ctx,
                        transforms,
                        &props,
                        ent_path,
                        world_from_obj,
                        highlights,
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
        handle_image_layering(scene);
    }
}
