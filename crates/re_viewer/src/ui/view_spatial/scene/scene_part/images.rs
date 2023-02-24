use std::sync::Arc;

use egui::NumExt;
use glam::Vec3;
use itertools::Itertools;

use re_data_store::{EntityPath, EntityProperties, InstancePathHash};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Tensor, TensorDataMeaning, TensorTrait},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::{
    renderer::{Volume2D, Volume2DDrawData},
    LineStripSeriesBuilder, Size,
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

struct DepthTexture<'a> {
    dimensions: glam::UVec2,
    tensor: &'a Tensor,
}
impl<'a> DepthTexture<'a> {
    pub fn from_tensor(tensor: &'a Tensor) -> Self {
        let (h, w) = (tensor.shape()[0].size, tensor.shape()[1].size);
        let dimensions = glam::UVec2::new(w as _, h as _);

        Self { dimensions, tensor }
    }

    pub fn get(&self, x: u32, y: u32) -> f32 {
        // TODO: is the depth texture..:
        // - linear?
        // - inversed?
        // - distance from camera plane or distance from camera?

        // TODO: that kind of normalization should be done in first pass

        // teardown
        // let (is_linear, n, f) = (false, 0.2, 500.0);
        // let is_reversed = false;

        // nyud
        let (is_linear, n, f) = (true, 0.2, 500.0);
        let is_reversed = false;

        // TODO: how does one do that with an infinite plane tho?
        fn depth_to_view_depth(n: f32, f: f32, z: f32) -> f32 {
            n * f / (f - z * (f - n))
        }
        fn view_depth_to_capped_linear(n: f32, f: f32, vz: f32) -> f32 {
            let vz = f32::min(vz, f);
            (vz - n) / (f - n)
        }
        fn linearize_depth(n: f32, f: f32, z: f32) -> f32 {
            view_depth_to_capped_linear(n, f, depth_to_view_depth(n, f, z))
        }

        let mut d = match &self.tensor.data {
            re_log_types::component_types::TensorData::U16(data) => {
                data.as_slice()[(x + y * self.dimensions.x) as usize] as f32 / u16::MAX as f32
            }
            re_log_types::component_types::TensorData::F32(data) => {
                data.as_slice()[(x + y * self.dimensions.x) as usize]
            }
            _ => todo!(),
        };

        if is_reversed {
            d = 1.0 - d;
        }
        if !is_linear {
            // d = linearize_depth(n, f, d);
            d = depth_to_view_depth(n, f, d);
            d = view_depth_to_capped_linear(n, f * 0.05, d);
        }

        d
    }
}

impl ImagesPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        entity_view: &EntityView<Tensor>,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
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

                let use_2d = true;

                if use_2d && tensor.meaning == TensorDataMeaning::Depth {
                    let depth = DepthTexture::from_tensor(&tensor);

                    let mut world_from_obj = world_from_obj;
                    world_from_obj.y_axis *= -1.0; // TODO
                    world_from_obj.z_axis *= -1.0; // TODO

                    let depth_size = depth.dimensions.as_vec2();
                    let (w, h) = (depth_size.x, depth_size.y);

                    let plane_distance = -world_from_obj.w_axis.z;
                    let volume_size_in_world = glam::Vec3::new(
                        world_from_obj.transform_vector3(glam::Vec3::X * w).x,
                        world_from_obj.transform_vector3(glam::Vec3::Y * h).y,
                        plane_distance,
                    );
                    dbg!(volume_size_in_world);

                    // let volume_size = glam::Vec3::new(w as f32, h as f32, w as f32 * 0.7) * 10.0;
                    let volume_dimensions = glam::UVec3::new(640, 640, 640) / 4;

                    let data = match &depth.tensor.data {
                        re_log_types::component_types::TensorData::U16(data) => data
                            .as_slice()
                            .iter()
                            .map(|d| *d as f32 / u16::MAX as f32)
                            .collect_vec(),
                        re_log_types::component_types::TensorData::F32(data) => {
                            data.as_slice().to_vec()
                        }
                        _ => todo!(),
                    };

                    {
                        let mut line_batch = scene.primitives.line_strips.batch("bbox");
                        let plane_distance = world_from_obj.w_axis.z;
                        line_batch.add_box_outline(
                            glam::Affine3A::from_scale_rotation_translation(
                                volume_size_in_world,
                                Default::default(),
                                glam::Vec3::new(0.0, 0.0, plane_distance * 0.5),
                            ),
                        );
                    }

                    // TODO: go through the new staging belt.
                    let volume_instances = vec![{
                        let scale = glam::Mat4::from_scale(volume_size_in_world);

                        let rotation = glam::Mat4::IDENTITY;

                        let translation_center = glam::Mat4::from_translation(
                            -glam::Vec3::new(0.5, 0.5, 1.0) * volume_size_in_world,
                        );

                        let world_from_model = rotation * translation_center * scale;
                        let model_from_world = world_from_model.inverse();

                        Volume2D {
                            world_from_model,
                            model_from_world,
                            dimensions: volume_dimensions,
                            depth_dimensions: depth.dimensions,
                            depth_data: data,
                            albedo_dimensions: depth.dimensions,
                            albedo_data: None,
                        }
                    }];

                    scene.primitives.volumes.extend(volume_instances);

                    break;
                }

                // if !use_2d && tensor.meaning == TensorDataMeaning::Depth {
                //     let mut world_from_obj = world_from_obj;
                //     world_from_obj.y_axis *= -1.0; // TODO
                //     world_from_obj.z_axis *= -1.0; // TODO

                //     let (h, w) = (tensor.shape()[0].size, tensor.shape()[1].size);
                //     eprintln!("rendering volumetric thing! {w}x{h}");

                //     let depth_dimensions = glam::UVec2::new(w as _, h as _);
                //     let depth_size = depth_dimensions.as_vec2();

                //     let plane_distance = -world_from_obj.w_axis.z;
                //     let volume_size_in_world = glam::Vec3::new(
                //         world_from_obj.transform_vector3(glam::Vec3::X * w as f32).x,
                //         world_from_obj.transform_vector3(glam::Vec3::Y * h as f32).y,
                //         plane_distance,
                //     );
                //     dbg!(volume_size_in_world);

                //     // let volume_size = glam::Vec3::new(w as f32, h as f32, w as f32 * 0.7) * 10.0;
                //     let volume_dimensions = glam::UVec3::new(640, 640, 640) / 4;

                //     let mut volume3d_rgba8 = vec![
                //         0u8;
                //         (volume_dimensions.x * volume_dimensions.y * volume_dimensions.z * 4)
                //             as usize
                //     ];

                //     let now = std::time::Instant::now();
                //     for (x, y) in (0..depth_dimensions.y)
                //         .flat_map(|y| (0..depth_dimensions.x).map(move |x| (x, y)))
                //     {
                //         let z = depth.get(x, y); // linear, near=0.0

                //         // Compute texture coordinates in the depth image's space.
                //         let texcoords = Vec2::new(x as f32, y as f32) / depth_size;

                //         // Compute texture coordinates in the volume's back panel space (z=1.0).
                //         // let texcoords_in_volume = texcoords.extend(1.0);
                //         let texcoords_in_volume = Vec3::new(texcoords.x, 1.0 - texcoords.y, 0.0);

                //         let cam_npos_in_volume = match *projection_kind {
                //             ProjectionKind::Orthographic => texcoords_in_volume.xy().extend(1.0),
                //             ProjectionKind::Perspective => Vec3::new(0.5, 0.5, 1.0),
                //         };

                //         let z = match (*projection_kind, *depth_kind) {
                //             (ProjectionKind::Orthographic, DepthKind::CameraPlane) => z,
                //             (ProjectionKind::Orthographic, DepthKind::CameraPosition) => {
                //                 // TODO: compute planar-based
                //                 z
                //             }
                //             (ProjectionKind::Perspective, DepthKind::CameraPlane) => {
                //                 // TODO: compute position-based
                //                 z
                //             }
                //             (ProjectionKind::Perspective, DepthKind::CameraPosition) => z,
                //         };

                //         let npos_in_volume =
                //             cam_npos_in_volume + (texcoords_in_volume - cam_npos_in_volume) * z;
                //         let pos_in_volume = npos_in_volume * (volume_dimensions.as_vec3() - 1.0);

                //         let pos = pos_in_volume.as_uvec3();

                //         let idx = (pos.x
                //             + pos.y * volume_dimensions.x
                //             + pos.z * volume_dimensions.x * volume_dimensions.y)
                //             as usize;
                //         let idx = idx * 4;

                //         let current = &volume3d_rgba8[idx..idx + 4];
                //         let color = albedo.get(x, y);
                //         let color = [
                //             ((color[0] as f32 + current[0] as f32) * 0.5) as u8,
                //             ((color[1] as f32 + current[1] as f32) * 0.5) as u8,
                //             ((color[2] as f32 + current[2] as f32) * 0.5) as u8,
                //             255,
                //         ];
                //         volume3d_rgba8[idx..idx + 4].copy_from_slice(&color);

                //         // let d = (z * 255.0) as u8;
                //         // faked[idx..idx + 4].copy_from_slice(&[d, d, d, 255]);
                //     }
                //     eprintln!("cpu time = {:?}", now.elapsed());
                // }

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
                    let rect =
                        glam::vec2(tensor.shape()[1].size as f32, tensor.shape()[0].size as f32);
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
