use egui::NumExt;
use glam::Vec3;
use itertools::Itertools;

use re_data_store::{query_latest_single, EntityPath, EntityProperties};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Tensor, TensorData, TensorDataMeaning},
    Component, DecodedTensor, Transform,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{
    renderer::{DepthCloud, DepthCloudAlbedoData, RectangleOptions},
    resource_managers::Texture2DCreationDesc,
    Colormap, OutlineMaskPreference,
};

use crate::{
    misc::{SpaceViewHighlights, SpaceViewOutlineMasks, TransformCache, ViewerContext},
    ui::{
        scene::SceneQuery,
        view_spatial::{Image, SceneSpatial},
        Annotations, DefaultColor,
    },
};

use super::ScenePart;

fn to_textured_rect(
    ctx: &mut ViewerContext<'_>,
    annotations: &Annotations,
    world_from_obj: glam::Mat4,
    ent_path: &EntityPath,
    tensor: &DecodedTensor,
    multiplicative_tint: egui::Rgba,
    outline_mask: OutlineMaskPreference,
) -> Option<re_renderer::renderer::TexturedRect> {
    crate::profile_function!();

    let Some([height, width, _]) = tensor.image_height_width_channels() else { return None; };

    let debug_name = ent_path.to_string();
    let tensor_stats = ctx.cache.tensor_stats(tensor);

    match crate::gpu_bridge::tensor_to_gpu(
        ctx.render_ctx,
        &debug_name,
        tensor,
        tensor_stats,
        annotations,
    ) {
        Ok(colormapped_texture) => {
            // TODO(emilk): let users pick texture filtering.
            // Always use nearest for magnification: let users see crisp individual pixels when they zoom
            let texture_filter_magnification = re_renderer::renderer::TextureFilterMag::Nearest;

            // For minimization: we want a smooth linear (ideally mipmapped) filter for color images.
            // Note that this filtering is done BEFORE applying the color map!
            // For labeled/annotated/class_Id images we want nearest, because interpolating classes makes no sense.
            // Interpolating depth images _can_ make sense, but can also produce weird artifacts when there are big jumps (0.1m -> 100m),
            // so it's usually safer to turn off.
            // The best heuristic is this: if there is a color map being applied, use nearest.
            // TODO(emilk): apply filtering _after_ the color map?
            let texture_filter_minification = if colormapped_texture.color_mapper.is_some() {
                re_renderer::renderer::TextureFilterMin::Nearest
            } else {
                re_renderer::renderer::TextureFilterMin::Linear
            };

            Some(re_renderer::renderer::TexturedRect {
                top_left_corner_position: world_from_obj.transform_point3(glam::Vec3::ZERO),
                extent_u: world_from_obj.transform_vector3(glam::Vec3::X * width as f32),
                extent_v: world_from_obj.transform_vector3(glam::Vec3::Y * height as f32),
                colormapped_texture,
                options: RectangleOptions {
                    texture_filter_magnification,
                    texture_filter_minification,
                    multiplicative_tint,
                    depth_offset: -1, // Push to background. Mostly important for mouse picking order!
                    outline_mask,
                },
            })
        }
        Err(err) => {
            re_log::error_once!("Failed to create texture from tensor for {debug_name:?}: {err}");
            None
        }
    }
}

fn handle_image_layering(scene: &mut SceneSpatial) {
    crate::profile_function!();

    // Handle layered rectangles that are on (roughly) the same plane and were logged in sequence.
    // First, group by similar plane.
    // TODO(andreas): Need planes later for picking as well!
    let images_grouped_by_plane = {
        let mut cur_plane = macaw::Plane3::from_normal_dist(Vec3::NAN, std::f32::NAN);
        let mut rectangle_group = Vec::new();
        scene
            .primitives
            .images
            .drain(..) // We rebuild the list as we might reorder as well!
            .batching(move |it| {
                for image in it {
                    let rect = &image.textured_rect;

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
                        let previous_group = std::mem::replace(&mut rectangle_group, vec![image]);
                        return Some(previous_group);
                    }
                    rectangle_group.push(image);
                }
                if !rectangle_group.is_empty() {
                    Some(rectangle_group.drain(..).collect())
                } else {
                    None
                }
            })
    }
    .collect_vec();

    // Then, for each planar group do resorting and change transparency.
    for mut grouped_images in images_grouped_by_plane {
        // Class id images should generally come last as they typically have large areas being zeroed out (which maps to fully transparent).
        grouped_images.sort_by_key(|image| image.tensor.meaning == TensorDataMeaning::ClassId);

        let total_num_images = grouped_images.len();
        for (idx, image) in grouped_images.iter_mut().enumerate() {
            // Set depth offset for correct order and avoid z fighting when there is a 3d camera.
            // Keep behind depth offset 0 for correct picking order.
            image.textured_rect.options.depth_offset =
                (idx as isize - total_num_images as isize) as re_renderer::DepthOffset;

            // make top images transparent
            let opacity = if idx == 0 {
                1.0
            } else {
                1.0 / total_num_images.at_most(20) as f32
            }; // avoid precision problems in framebuffer
            image.textured_rect.options.multiplicative_tint = image
                .textured_rect
                .options
                .multiplicative_tint
                .multiply(opacity);
        }

        scene.primitives.images.extend(grouped_images);
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
        properties: &mut EntityProperties,
        ent_path: &EntityPath,
        world_from_obj: glam::Mat4,
        highlights: &SpaceViewHighlights,
    ) -> Result<(), QueryError> {
        crate::profile_function!();

        // Instance ids of tensors refer to entries inside the tensor.
        for (tensor, color) in itertools::izip!(
            entity_view.iter_primary()?,
            entity_view.iter_component::<ColorRGBA>()?
        ) {
            crate::profile_scope!("loop_iter");
            let Some(tensor) = tensor else { continue; };

            if !tensor.is_shaped_like_an_image() {
                return Ok(());
            }

            let tensor = match ctx.cache.decode.try_decode_tensor_if_necessary(tensor) {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {ent_path}: {err}"
                    );
                    continue;
                }
            };

            let annotations = scene.annotation_map.find(ent_path);
            let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

            if *properties.backproject_depth.get() && tensor.meaning == TensorDataMeaning::Depth {
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
                        ent_path,
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

            let color = annotations.class_description(None).annotation_info().color(
                color.map(|c| c.to_array()).as_ref(),
                DefaultColor::OpaqueWhite,
            );

            if let Some(textured_rect) = to_textured_rect(
                ctx,
                &annotations,
                world_from_obj,
                ent_path,
                &tensor,
                color.into(),
                entity_highlight.overall,
            ) {
                scene.primitives.images.push(Image {
                    ent_path: ent_path.clone(),
                    tensor,
                    textured_rect,
                });
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view_as_depth_cloud(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        transforms: &TransformCache,
        properties: &mut EntityProperties,
        tensor: &DecodedTensor,
        ent_path: &EntityPath,
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

        let Some([height, width, _]) = tensor.image_height_width_channels() else {
            return Err(format!("Tensor at {ent_path:?} is not an image"));
        };
        let dimensions = glam::UVec2::new(width as _, height as _);

        let depth_texture = {
            // Ideally, we'd use the same key as for displaying the texture, but we might make other compromises regarding formats etc.!
            // So to not couple this, we use a different key here
            let texture_key = egui::util::hash((tensor.id(), "depth_cloud"));
            let mut data_f32 = Vec::new();
            ctx.render_ctx
                .texture_manager_2d
                .get_or_try_create_with(
                    texture_key,
                    &mut ctx.render_ctx.gpu_resources.textures,
                    || {
                        // TODO(andreas/cmc): Ideally we'd upload the u16 data as-is.
                        // However, R16Unorm is behind a feature flag and Depth16Unorm doesn't work on WebGL (and is awkward as this is a depth buffer format!).
                        let data = match &tensor.data {
                            TensorData::U16(data) => {
                                data_f32.extend(data.as_slice().iter().map(|d| *d as f32));
                                bytemuck::cast_slice(&data_f32).into()
                            }
                            TensorData::F32(data) => bytemuck::cast_slice(data).into(),
                            _ => {
                                return Err(format!(
                                    "Tensor datatype {} is not supported for back-projection",
                                    tensor.dtype()
                                ));
                            }
                        };

                        Ok(Texture2DCreationDesc {
                            label: format!("Depth cloud for {ent_path:?}").into(),
                            data,
                            format: wgpu::TextureFormat::R32Float,
                            width: width as _,
                            height: height as _,
                        })
                    },
                )
                .map_err(|err| format!("Failed to create depth cloud texture: {err}"))?
        };

        let depth_from_world_scale = *properties.depth_from_world_scale.get();

        let world_depth_from_texture_depth = 1.0 / depth_from_world_scale;

        let mut colormap = match *properties.color_mapper.get() {
            re_data_store::ColorMapper::Colormap(colormap) => match colormap {
                re_data_store::Colormap::Grayscale => Colormap::Grayscale,
                re_data_store::Colormap::Turbo => Colormap::Turbo,
                re_data_store::Colormap::Viridis => Colormap::Viridis,
                re_data_store::Colormap::Plasma => Colormap::Plasma,
                re_data_store::Colormap::Magma => Colormap::Magma,
                re_data_store::Colormap::Inferno => Colormap::Inferno,
            },
            re_data_store::ColorMapper::AlbedoTexture => Colormap::AlbedoTexture,
        };

        let mut albedo_data = None;
        let mut albedo_dimensions = glam::UVec2::ZERO;

        if colormap == Colormap::AlbedoTexture {
            let tensor = properties.albedo_texture.as_ref().and_then(|path| {
                query_latest_single::<Tensor>(&ctx.log_db.entity_db, path, &ctx.current_query())
            });
            if let Some(tensor) = tensor {
                let (h, w) = (tensor.shape()[0].size, tensor.shape()[1].size);
                albedo_dimensions = glam::UVec2::new(w as _, h as _);

                // TODO(cmc): How does one know whether the texture is sRGB or not at this point?
                // TODO(cmc): We should easily be able to pass almost any datatype here.

                albedo_data = match &tensor.data {
                    TensorData::U8(data) => {
                        if let Some([_, _, c]) = tensor.image_height_width_channels() {
                            match c {
                                1 => Some(DepthCloudAlbedoData::Mono8(data.0.to_vec())),
                                3 => Some(DepthCloudAlbedoData::Rgb8(data.0.to_vec())),
                                4 => Some(DepthCloudAlbedoData::Rgb8Srgb(data.0.to_vec())),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    _ => {
                        re_log::debug_once!(
                            "Tensor datatype not supported for albedo texture ({:?})",
                            std::mem::discriminant(&tensor.data),
                        );
                        None
                    }
                };
            } else {
                re_log::debug_once!(
                    "Albedo texture couldn't be fetched ({:?})",
                    properties.albedo_texture
                );
                colormap = Colormap::Grayscale;
            }
        }

        // We want point radius to be defined in a scale where the radius of a point
        // is a factor (`backproject_radius_scale`) of the diameter of a pixel projected
        // at that distance.
        let fov_y = intrinsics.fov_y().unwrap_or(1.0);
        let pixel_width_from_depth = (0.5 * fov_y).tan() / (0.5 * height as f32);
        let radius_scale = *properties.backproject_radius_scale.get();
        let point_radius_from_world_depth = radius_scale * pixel_width_from_depth;

        let max_data_value = if let Some((_min, max)) = ctx.cache.tensor_stats(tensor).range {
            max as f32
        } else {
            // This could only happen for Jpegs, and we should never get here.
            // TODO(emilk): refactor the code so that we can always calculate a range for the tensor
            re_log::warn_once!("Couldn't calculate range for a depth tensor!?");
            match tensor.data {
                TensorData::U16(_) => u16::MAX as f32,
                _ => 10.0,
            }
        };

        scene.primitives.depth_clouds.clouds.push(DepthCloud {
            world_from_obj,
            depth_camera_intrinsics: intrinsics.image_from_cam.into(),
            world_depth_from_texture_depth,
            point_radius_from_world_depth,
            max_depth_in_world: max_data_value / depth_from_world_scale,
            depth_dimensions: dimensions,
            depth_texture,
            colormap,
            outline_mask_id: entity_highlight.overall,
            picking_object_id: re_renderer::PickingLayerObjectId(ent_path.hash64()),
            albedo_data,
            albedo_dimensions,
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

        for (ent_path, mut props) in query.iter_entities() {
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
                        &mut props,
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
