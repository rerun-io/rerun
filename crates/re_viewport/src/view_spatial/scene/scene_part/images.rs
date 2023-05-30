use std::collections::BTreeMap;

use egui::NumExt;

use re_components::{
    ColorRGBA, Component as _, DecodedTensor, DrawOrder, InstanceKey, Pinhole, Tensor, TensorData,
    TensorDataMeaning,
};
use re_data_store::{EntityPath, EntityProperties};
use re_log_types::EntityPathHash;
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{
    renderer::{DepthCloud, RectangleOptions},
    resource_managers::Texture2DCreationDesc,
    Colormap, OutlineMaskPreference,
};
use re_viewer_context::{
    gpu_bridge, Annotations, DefaultColor, SceneQuery, TensorDecodeCache, TensorStatsCache,
    ViewerContext,
};

use crate::{
    space_view_highlights::{SpaceViewHighlights, SpaceViewOutlineMasks},
    transform_cache::TransformCache,
    view_spatial::{scene::EntityDepthOffsets, Image, SceneSpatial},
};

use super::ScenePart;

#[allow(clippy::too_many_arguments)]
fn to_textured_rect(
    ctx: &mut ViewerContext<'_>,
    annotations: &Annotations,
    world_from_obj: glam::Affine3A,
    ent_path: &EntityPath,
    tensor: &DecodedTensor,
    multiplicative_tint: egui::Rgba,
    outline_mask: OutlineMaskPreference,
    depth_offset: re_renderer::DepthOffset,
) -> Option<re_renderer::renderer::TexturedRect> {
    crate::profile_function!();

    let Some([height, width, _]) = tensor.image_height_width_channels() else { return None; };

    let debug_name = ent_path.to_string();
    let tensor_stats = ctx.cache.entry::<TensorStatsCache>().entry(tensor);

    match gpu_bridge::tensor_to_gpu(
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
                    depth_offset,
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ImageGrouping {
    parent_pinhole: Option<EntityPathHash>,
    draw_order: DrawOrder,
}

fn handle_image_layering(scene: &mut SceneSpatial) {
    crate::profile_function!();

    // Rebuild the image list, grouped by "shared plane", identified with camera & draw order.
    let mut image_groups: BTreeMap<ImageGrouping, Vec<Image>> = BTreeMap::new();
    for image in scene.primitives.images.drain(..) {
        image_groups
            .entry(ImageGrouping {
                parent_pinhole: image.parent_pinhole,
                draw_order: image.draw_order,
            })
            .or_default()
            .push(image);
    }

    // Then, for each group do resorting and change transparency.
    for (_, mut images) in image_groups {
        // Since we change transparency depending on order and re_renderer doesn't handle transparency
        // ordering either, we need to ensure that sorting is stable at the very least.
        // Sorting is done by depth offset, not by draw order which is the same for the entire group.
        //
        // Class id images should generally come last within the same layer as
        // they typically have large areas being zeroed out (which maps to fully transparent).
        images.sort_by_key(|image| {
            (
                image.textured_rect.options.depth_offset,
                image.tensor.meaning == TensorDataMeaning::ClassId,
            )
        });

        let total_num_images = images.len();
        for (idx, image) in images.iter_mut().enumerate() {
            // make top images transparent
            let opacity = if idx == 0 {
                1.0
            } else {
                // avoid precision problems in framebuffer
                1.0 / total_num_images.at_most(20) as f32
            };
            image.textured_rect.options.multiplicative_tint = image
                .textured_rect
                .options
                .multiplicative_tint
                .multiply(opacity);
        }

        scene.primitives.images.extend(images);
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
        world_from_obj: glam::Affine3A,
        highlights: &SpaceViewHighlights,
        depth_offset: re_renderer::DepthOffset,
    ) -> Result<(), QueryError> {
        crate::profile_function!();

        // Instance ids of tensors refer to entries inside the tensor.
        for (tensor, color, draw_order) in itertools::izip!(
            entity_view.iter_primary()?,
            entity_view.iter_component::<ColorRGBA>()?,
            entity_view.iter_component::<DrawOrder>()?
        ) {
            crate::profile_scope!("loop_iter");
            let Some(tensor) = tensor else { continue; };

            if !tensor.is_shaped_like_an_image() {
                return Ok(());
            }

            let tensor = match ctx.cache.entry::<TensorDecodeCache>().entry(tensor) {
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
                let closet_pinhole = ctx
                    .log_db
                    .entity_db
                    .data_store
                    .query_latest_component_at_closest_ancestor::<Pinhole>(ent_path, &query);

                if let Some((pinhole_ent_path, _)) = closet_pinhole {
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
                depth_offset,
            ) {
                scene.primitives.images.push(Image {
                    ent_path: ent_path.clone(),
                    tensor,
                    textured_rect,
                    parent_pinhole: transforms.parent_pinhole(ent_path),
                    draw_order: draw_order.unwrap_or(DrawOrder::DEFAULT_IMAGE),
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
        properties: &EntityProperties,
        tensor: &DecodedTensor,
        ent_path: &EntityPath,
        pinhole_ent_path: &EntityPath,
        entity_highlight: &SpaceViewOutlineMasks,
    ) -> Result<(), String> {
        crate::profile_function!();

        let store = &ctx.log_db.entity_db.data_store;
        let Some(intrinsics) = store.query_latest_component::<Pinhole>(
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

        let colormap = match *properties.color_mapper.get() {
            re_data_store::ColorMapper::Colormap(colormap) => match colormap {
                re_data_store::Colormap::Grayscale => Colormap::Grayscale,
                re_data_store::Colormap::Turbo => Colormap::Turbo,
                re_data_store::Colormap::Viridis => Colormap::Viridis,
                re_data_store::Colormap::Plasma => Colormap::Plasma,
                re_data_store::Colormap::Magma => Colormap::Magma,
                re_data_store::Colormap::Inferno => Colormap::Inferno,
            },
        };

        // We want point radius to be defined in a scale where the radius of a point
        // is a factor (`backproject_radius_scale`) of the diameter of a pixel projected
        // at that distance.
        let fov_y = intrinsics.fov_y().unwrap_or(1.0);
        let pixel_width_from_depth = (0.5 * fov_y).tan() / (0.5 * height as f32);
        let radius_scale = *properties.backproject_radius_scale.get();
        let point_radius_from_world_depth = radius_scale * pixel_width_from_depth;

        let max_data_value =
            if let Some((_min, max)) = ctx.cache.entry::<TensorStatsCache>().entry(tensor).range {
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
            world_from_obj: world_from_obj.into(),
            depth_camera_intrinsics: intrinsics.image_from_cam.into(),
            world_depth_from_texture_depth,
            point_radius_from_world_depth,
            max_depth_in_world: max_data_value / depth_from_world_scale,
            depth_dimensions: dimensions,
            depth_texture,
            colormap,
            outline_mask_id: entity_highlight.overall,
            picking_object_id: re_renderer::PickingLayerObjectId(ent_path.hash64()),
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
        depth_offsets: &EntityDepthOffsets,
    ) {
        crate::profile_scope!("ImagesPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };

            match query_primary_with_history::<Tensor, 4>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    Tensor::name(),
                    InstanceKey::name(),
                    ColorRGBA::name(),
                    DrawOrder::name(),
                ],
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
                        depth_offsets.get(ent_path).unwrap_or(depth_offsets.image),
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
