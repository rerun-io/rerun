use std::sync::Arc;

use egui::NumExt;
use glam::Vec3;
use itertools::Itertools;

use re_data_store::{query_latest_single, EntityPath, EntityProperties, InstancePathHash};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Tensor, TensorData, TensorDataMeaning, TensorTrait},
    Component, Transform,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{
    renderer::{DepthCloud, DepthCloudDepthData, OutlineMaskPreference},
    ColorMap,
};

use crate::{
    misc::{SpaceViewHighlights, SpaceViewOutlineMasks, TransformCache, ViewerContext},
    ui::{
        scene::SceneQuery,
        view_spatial::{scene::scene_part::instance_path_hash_for_picking, Image, SceneSpatial},
        Annotations, DefaultColor,
    },
};

use super::ScenePart;

#[allow(clippy::too_many_arguments)]
fn push_tensor_texture(
    scene: &mut SceneSpatial,
    ctx: &mut ViewerContext<'_>,
    annotations: &Arc<Annotations>,
    world_from_obj: glam::Mat4,
    instance_path_hash: InstancePathHash,
    tensor: &Tensor,
    tint: egui::Rgba,
    outline_mask: OutlineMaskPreference,
) {
    crate::profile_function!();

    let tensor_view = ctx.cache.image.get_colormapped_view(tensor, annotations);

    if let Some(texture_handle) = tensor_view.texture_handle(ctx.render_ctx) {
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
                outline_mask,
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

                let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

                if *properties.backproject_depth.get() && tensor.meaning == TensorDataMeaning::Depth
                {
                    let query = ctx.current_query();
                    let pinhole_ent_path =
                        crate::misc::queries::closest_pinhole_transform(ctx, ent_path, &query);

                    if let Some(pinhole_ent_path) = pinhole_ent_path {
                        // NOTE: we don't pass in `world_from_obj` because this corresponds to the
                        // transform of the projection plane, which is of no use to us here.
                        // What we want are the extrinsics of the depth camera!
                        match Self::process_entity_view_as_depth_cloud(
                            scene,
                            ctx,
                            transforms,
                            properties,
                            &tensor,
                            &pinhole_ent_path,
                            entity_highlight,
                        ) {
                            Ok(()) => return Ok(()),
                            Err(err) => {
                                re_log::warn_once!("{err}");
                            }
                        }
                    };
                }

                Self::process_entity_view_as_image(
                    entity_view,
                    scene,
                    ctx,
                    properties,
                    ent_path,
                    world_from_obj,
                    entity_highlight,
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
        entity_highlight: &SpaceViewOutlineMasks,
        instance_key: InstanceKey,
        tensor: Tensor,
        color: Option<ColorRGBA>,
    ) {
        crate::profile_function!();

        let instance_path_hash = instance_path_hash_for_picking(
            ent_path,
            instance_key,
            entity_view,
            properties,
            entity_highlight.any_selection_highlight,
        );

        let annotations = scene.annotation_map.find(ent_path);

        let color = annotations.class_description(None).annotation_info().color(
            color.map(|c| c.to_array()).as_ref(),
            DefaultColor::OpaqueWhite,
        );

        let outline_mask = entity_highlight.index_outline_mask(instance_path_hash.instance_key);

        match ctx.cache.decode.try_decode_tensor_if_necessary(tensor) {
            Ok(tensor) => {
                push_tensor_texture(
                    scene,
                    ctx,
                    &annotations,
                    world_from_obj,
                    instance_path_hash,
                    &tensor,
                    color.into(),
                    outline_mask,
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
            Err(err) => {
                // TODO(jleibs): Would be nice to surface these through the UI instead
                re_log::warn_once!(
                    "Encountered problem decoding tensor at path {}: {}",
                    ent_path,
                    err
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view_as_depth_cloud(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        transforms: &TransformCache,
        properties: &EntityProperties,
        tensor: &Tensor,
        pinhole_ent_path: &EntityPath,
        entity_highlight: &SpaceViewOutlineMasks,
    ) -> Result<(), String> {
        crate::profile_function!();

        let Some(re_log_types::Transform::Pinhole(intrinsics)) = query_latest_single::<Transform>(
            &ctx.log_db.entity_db,
            pinhole_ent_path,
            &ctx.current_query(),
        ) else {
            return Err(format!("Couldn't fetch pinhole intrinsics at {pinhole_ent_path:?}"));
        };

        // TODO(cmc): getting to those extrinsics is no easy task :|
        let world_from_obj = pinhole_ent_path
            .parent()
            .and_then(|ent_path| transforms.reference_from_entity(&ent_path));
        let Some(world_from_obj) = world_from_obj else {
            return Err(format!("Couldn't fetch pinhole extrinsics at {pinhole_ent_path:?}"));
        };

        // TODO(cmc): automagically convert as needed for non-natively supported datatypes?
        let data = match &tensor.data {
            // NOTE: Shallow clone if feature `arrow` is enabled, full alloc + memcpy otherwise.
            TensorData::U16(data) => DepthCloudDepthData::U16(data.clone()),
            TensorData::F32(data) => DepthCloudDepthData::F32(data.clone()),
            _ => {
                return Err(format!(
                    "Tensor datatype {} is not supported for backprojection",
                    tensor.dtype()
                ));
            }
        };

        let depth_from_world_scale = *properties.depth_from_world_scale.get();
        let world_depth_from_data_depth = 1.0 / depth_from_world_scale;

        let (h, w) = (tensor.shape()[0].size, tensor.shape()[1].size);
        let dimensions = glam::UVec2::new(w as _, h as _);

        let colormap = match *properties.color_mapper.get() {
            re_data_store::ColorMapper::ColorMap(colormap) => match colormap {
                re_data_store::ColorMap::Grayscale => ColorMap::Grayscale,
                re_data_store::ColorMap::Turbo => ColorMap::ColorMapTurbo,
                re_data_store::ColorMap::Viridis => ColorMap::ColorMapViridis,
                re_data_store::ColorMap::Plasma => ColorMap::ColorMapPlasma,
                re_data_store::ColorMap::Magma => ColorMap::ColorMapMagma,
                re_data_store::ColorMap::Inferno => ColorMap::ColorMapInferno,
            },
        };

        // We want point radius to be defined in a scale where the radius of a point
        // is a factor (`backproject_radius_scale`) of the diameter of a pixel projected
        // at that distance.
        let fov_y = intrinsics.fov_y().unwrap_or(1.0);
        let pixel_width_from_depth = (0.5 * fov_y).tan() / (0.5 * h as f32);
        let radius_scale = *properties.backproject_radius_scale.get();
        let point_radius_from_world_depth = radius_scale * pixel_width_from_depth;

        let max_data_value = if let Some((_min, max)) = ctx.cache.tensor_stats(tensor).range {
            max as f32
        } else {
            // This could only happen for Jpegs, and we should never get here.
            // TODO(emilk): refactor the code so that we can always calculate a range for the tensor
            re_log::warn_once!("Couldn't calculate range for a depth tensor!?");
            match data {
                DepthCloudDepthData::U16(_) => u16::MAX as f32,
                DepthCloudDepthData::F32(_) => 10.0,
            }
        };

        scene.primitives.depth_clouds.push(DepthCloud {
            world_from_obj,
            depth_camera_intrinsics: intrinsics.image_from_cam.into(),
            world_depth_from_data_depth,
            point_radius_from_world_depth,
            max_depth_in_world: world_depth_from_data_depth * max_data_value,
            depth_dimensions: dimensions,
            depth_data: data,
            colormap,
            outline_mask_id: entity_highlight.overall,
            size_boost_in_points_for_outlines:
                crate::ui::view_spatial::scene::primitives::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        });

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
