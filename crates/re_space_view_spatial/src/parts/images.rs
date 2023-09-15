use std::collections::BTreeMap;

use egui::NumExt;
use itertools::Itertools as _;
use nohash_hasher::IntSet;

use re_arrow_store::LatestAtQuery;
use re_components::Pinhole;
use re_data_store::{EntityPath, EntityProperties, InstancePathHash, VersionedInstancePathHash};
use re_log_types::{EntityPathHash, TimeInt, Timeline};
use re_query::{ArchetypeView, QueryError};
use re_renderer::{
    renderer::{DepthCloud, DepthClouds, RectangleOptions, TexturedRect},
    Colormap,
};
use re_types::{
    archetypes::{DepthImage, Image, SegmentationImage},
    components::{Color, DrawOrder, TensorData},
    tensor_data::{DecodedTensor, TensorDataMeaning},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    default_heuristic_filter, gpu_bridge, DefaultColor, SpaceViewSystemExecutionError,
    TensorDecodeCache, TensorStatsCache, ViewPartSystem, ViewQuery, ViewerContext,
};
use re_viewer_context::{NamedViewSystem, ViewContextCollection};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext, TransformContext},
    parts::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
    view_kind::SpatialSpaceViewKind,
};

use super::{entity_iterator::process_archetype_views, SpatialViewPartData};

pub struct ViewerImage {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    /// The meaning of the tensor stored in the image
    pub meaning: TensorDataMeaning,

    pub tensor: DecodedTensor,

    /// Textured rectangle for the renderer.
    pub textured_rect: TexturedRect,

    /// Pinhole camera this image is under.
    pub parent_pinhole: Option<EntityPathHash>,

    /// Draw order value used.
    pub draw_order: DrawOrder,
}

#[allow(clippy::too_many_arguments)]
fn to_textured_rect(
    ctx: &mut ViewerContext<'_>,
    ent_path: &EntityPath,
    ent_context: &SpatialSceneEntityContext<'_>,
    tensor_path_hash: VersionedInstancePathHash,
    tensor: &DecodedTensor,
    meaning: TensorDataMeaning,
    multiplicative_tint: egui::Rgba,
) -> Option<re_renderer::renderer::TexturedRect> {
    re_tracing::profile_function!();

    let Some([height, width, _]) = tensor.image_height_width_channels() else {
        return None;
    };

    let debug_name = ent_path.to_string();
    let tensor_stats = ctx
        .cache
        .entry(|c: &mut TensorStatsCache| c.entry(tensor_path_hash, tensor));

    match gpu_bridge::tensor_to_gpu(
        ctx.render_ctx,
        &debug_name,
        tensor_path_hash,
        tensor,
        meaning,
        &tensor_stats,
        &ent_context.annotations,
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
                top_left_corner_position: ent_context
                    .world_from_obj
                    .transform_point3(glam::Vec3::ZERO),
                extent_u: ent_context
                    .world_from_obj
                    .transform_vector3(glam::Vec3::X * width as f32),
                extent_v: ent_context
                    .world_from_obj
                    .transform_vector3(glam::Vec3::Y * height as f32),
                colormapped_texture,
                options: RectangleOptions {
                    texture_filter_magnification,
                    texture_filter_minification,
                    multiplicative_tint,
                    depth_offset: ent_context.depth_offset,
                    outline_mask: ent_context.highlight.overall,
                },
            })
        }
        Err(err) => {
            re_log::error_once!("Failed to create texture from tensor for {debug_name:?}: {err}");
            None
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ImageGrouping {
    parent_pinhole: Option<EntityPathHash>,
    draw_order: DrawOrder,
}

pub struct ImagesPart {
    pub data: SpatialViewPartData,
    pub images: Vec<ViewerImage>,
    pub depth_cloud_entities: IntSet<EntityPathHash>,
}

impl Default for ImagesPart {
    fn default() -> Self {
        Self {
            data: SpatialViewPartData::new(Some(SpatialSpaceViewKind::TwoD)),
            images: Vec::new(),
            depth_cloud_entities: IntSet::default(),
        }
    }
}

impl ImagesPart {
    fn handle_image_layering(&mut self) {
        re_tracing::profile_function!();

        // Rebuild the image list, grouped by "shared plane", identified with camera & draw order.
        let mut image_groups: BTreeMap<ImageGrouping, Vec<ViewerImage>> = BTreeMap::new();
        for image in self.images.drain(..) {
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
                    image.meaning == TensorDataMeaning::ClassId,
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

            self.images.extend(images);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_image_arch_view(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        transforms: &TransformContext,
        _ent_props: &EntityProperties,
        arch_view: &ArchetypeView<Image>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let parent_pinhole_path = transforms.parent_pinhole(ent_path);

        // If this isn't an image, return
        // TODO(jleibs): The ArchetypeView should probably do this for us.
        if !ctx.store_db.store().entity_has_component(
            &ctx.current_query().timeline,
            ent_path,
            &Image::indicator_component(),
        ) {
            return Ok(());
        }
        // Unknown is currently interpreted as "Some Color" in most cases.
        // TODO(jleibs): Make this more explicit
        let meaning = TensorDataMeaning::Unknown;

        // Instance ids of tensors refer to entries inside the tensor.
        for (tensor, color, draw_order) in itertools::izip!(
            arch_view.iter_required_component::<TensorData>()?,
            arch_view.iter_optional_component::<Color>()?,
            arch_view.iter_optional_component::<DrawOrder>()?
        ) {
            re_tracing::profile_scope!("loop_iter");

            if !tensor.is_shaped_like_an_image() {
                return Ok(());
            }

            // NOTE: Tensors don't support batches at the moment so always splat.
            let tensor_path_hash =
                InstancePathHash::entity_splat(ent_path).versioned(arch_view.primary_row_id());
            let tensor = match ctx
                .cache
                .entry(|c: &mut TensorDecodeCache| c.entry(tensor_path_hash, tensor.0))
            {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {ent_path}: {err}"
                    );
                    continue;
                }
            };

            let color = ent_context
                .annotations
                .resolved_class_description(None)
                .annotation_info()
                .color(
                    color.map(|c| c.to_array()).as_ref(),
                    DefaultColor::OpaqueWhite,
                );

            if let Some(textured_rect) = to_textured_rect(
                ctx,
                ent_path,
                ent_context,
                tensor_path_hash,
                &tensor,
                meaning,
                color.into(),
            ) {
                self.extend_bbox(&textured_rect);

                self.images.push(ViewerImage {
                    ent_path: ent_path.clone(),
                    tensor,
                    meaning,
                    textured_rect,
                    parent_pinhole: parent_pinhole_path.map(|p| p.hash()),
                    draw_order: draw_order.unwrap_or(DrawOrder::DEFAULT_IMAGE),
                });
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn process_depth_image_arch_view(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        depth_clouds: &mut Vec<DepthCloud>,
        transforms: &TransformContext,
        ent_props: &EntityProperties,
        arch_view: &ArchetypeView<DepthImage>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        // If this isn't an image, return
        // TODO(jleibs): The ArchetypeView should probably to this for us.
        if !ctx.store_db.store().entity_has_component(
            &ctx.current_query().timeline,
            ent_path,
            &DepthImage::indicator_component(),
        ) {
            return Ok(());
        }
        let meaning = TensorDataMeaning::Depth;

        let parent_pinhole_path = transforms.parent_pinhole(ent_path);

        // Instance ids of tensors refer to entries inside the tensor.
        for (tensor, color, draw_order) in itertools::izip!(
            arch_view.iter_required_component::<TensorData>()?,
            arch_view.iter_optional_component::<Color>()?,
            arch_view.iter_optional_component::<DrawOrder>()?
        ) {
            // NOTE: we ignore the `DepthMeter` component here because we get it from
            // `EntityProperties::depth_from_world_scale` instead, which is initialized to the
            // same value, but the user may have edited it.
            re_tracing::profile_scope!("loop_iter");

            if !tensor.is_shaped_like_an_image() {
                return Ok(());
            }

            // NOTE: Tensors don't support batches at the moment so always splat.
            let tensor_path_hash =
                InstancePathHash::entity_splat(ent_path).versioned(arch_view.primary_row_id());
            let tensor = match ctx
                .cache
                .entry(|c: &mut TensorDecodeCache| c.entry(tensor_path_hash, tensor.0))
            {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {ent_path}: {err}"
                    );
                    continue;
                }
            };

            if *ent_props.backproject_depth {
                if let Some(parent_pinhole_path) = transforms.parent_pinhole(ent_path) {
                    // NOTE: we don't pass in `world_from_obj` because this corresponds to the
                    // transform of the projection plane, which is of no use to us here.
                    // What we want are the extrinsics of the depth camera!
                    match Self::process_entity_view_as_depth_cloud(
                        ctx,
                        transforms,
                        ent_context,
                        ent_props,
                        tensor_path_hash,
                        &tensor,
                        ent_path,
                        parent_pinhole_path,
                    ) {
                        Ok(cloud) => {
                            self.data
                                .extend_bounding_box(cloud.bbox(), cloud.world_from_rdf);
                            self.depth_cloud_entities.insert(ent_path.hash());
                            depth_clouds.push(cloud);
                            return Ok(());
                        }
                        Err(err) => {
                            re_log::warn_once!("{err}");
                        }
                    }
                };
            }

            let color = ent_context
                .annotations
                .resolved_class_description(None)
                .annotation_info()
                .color(
                    color.map(|c| c.to_array()).as_ref(),
                    DefaultColor::OpaqueWhite,
                );

            if let Some(textured_rect) = to_textured_rect(
                ctx,
                ent_path,
                ent_context,
                tensor_path_hash,
                &tensor,
                meaning,
                color.into(),
            ) {
                self.extend_bbox(&textured_rect);

                self.images.push(ViewerImage {
                    ent_path: ent_path.clone(),
                    tensor,
                    meaning,
                    textured_rect,
                    parent_pinhole: parent_pinhole_path.map(|p| p.hash()),
                    draw_order: draw_order.unwrap_or(DrawOrder::DEFAULT_IMAGE),
                });
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn process_segmentation_image_arch_view(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        transforms: &TransformContext,
        _ent_props: &EntityProperties,
        arch_view: &ArchetypeView<SegmentationImage>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let parent_pinhole_path = transforms.parent_pinhole(ent_path);

        // If this isn't an image, return
        // TODO(jleibs): The ArchetypeView should probably to this for us.
        if !ctx.store_db.store().entity_has_component(
            &ctx.current_query().timeline,
            ent_path,
            &SegmentationImage::indicator_component(),
        ) {
            return Ok(());
        }

        let meaning = TensorDataMeaning::ClassId;

        // Instance ids of tensors refer to entries inside the tensor.
        for (tensor, color, draw_order) in itertools::izip!(
            arch_view.iter_required_component::<TensorData>()?,
            arch_view.iter_optional_component::<Color>()?,
            arch_view.iter_optional_component::<DrawOrder>()?
        ) {
            re_tracing::profile_scope!("loop_iter");

            if !tensor.is_shaped_like_an_image() {
                return Ok(());
            }

            // NOTE: Tensors don't support batches at the moment so always splat.
            let tensor_path_hash =
                InstancePathHash::entity_splat(ent_path).versioned(arch_view.primary_row_id());
            let tensor = match ctx
                .cache
                .entry(|c: &mut TensorDecodeCache| c.entry(tensor_path_hash, tensor.0))
            {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {ent_path}: {err}"
                    );
                    continue;
                }
            };

            let color = ent_context
                .annotations
                .resolved_class_description(None)
                .annotation_info()
                .color(
                    color.map(|c| c.to_array()).as_ref(),
                    DefaultColor::OpaqueWhite,
                );

            if let Some(textured_rect) = to_textured_rect(
                ctx,
                ent_path,
                ent_context,
                tensor_path_hash,
                &tensor,
                meaning,
                color.into(),
            ) {
                self.extend_bbox(&textured_rect);

                self.images.push(ViewerImage {
                    ent_path: ent_path.clone(),
                    tensor,
                    meaning,
                    textured_rect,
                    parent_pinhole: parent_pinhole_path.map(|p| p.hash()),
                    draw_order: draw_order.unwrap_or(DrawOrder::DEFAULT_IMAGE),
                });
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view_as_depth_cloud(
        ctx: &mut ViewerContext<'_>,
        transforms: &TransformContext,
        ent_context: &SpatialSceneEntityContext<'_>,
        properties: &EntityProperties,
        tensor_path_hash: VersionedInstancePathHash,
        tensor: &DecodedTensor,
        ent_path: &EntityPath,
        parent_pinhole_path: &EntityPath,
    ) -> anyhow::Result<DepthCloud> {
        re_tracing::profile_function!();

        let store = &ctx.store_db.entity_db.data_store;

        let Some(intrinsics) =
            store.query_latest_component::<Pinhole>(parent_pinhole_path, &ctx.current_query())
        else {
            anyhow::bail!("Couldn't fetch pinhole intrinsics at {parent_pinhole_path:?}");
        };

        let view_coordinates = crate::contexts::pinhole_camera_view_coordinates(
            ctx.store_db.store(),
            &ctx.current_query(),
            parent_pinhole_path,
        );

        // TODO(cmc): getting to those extrinsics is no easy task :|
        let world_from_view = parent_pinhole_path
            .parent()
            .and_then(|ent_path| transforms.reference_from_entity(&ent_path));
        let Some(world_from_view) = world_from_view else {
            anyhow::bail!("Couldn't fetch pinhole extrinsics at {parent_pinhole_path:?}");
        };
        let world_from_rdf =
            world_from_view * glam::Affine3A::from_mat3(view_coordinates.from_rdf());

        let Some([height, width, _]) = tensor.image_height_width_channels() else {
            anyhow::bail!("Tensor at {ent_path:?} is not an image");
        };
        let dimensions = glam::UVec2::new(width as _, height as _);

        let debug_name = ent_path.to_string();
        let tensor_stats = ctx
            .cache
            .entry(|c: &mut TensorStatsCache| c.entry(tensor_path_hash, tensor));
        let depth_texture = re_viewer_context::gpu_bridge::depth_tensor_to_gpu(
            ctx.render_ctx,
            &debug_name,
            tensor_path_hash,
            tensor,
            &tensor_stats,
        )?;

        let depth_from_world_scale = *properties.depth_from_world_scale;

        let world_depth_from_texture_depth = 1.0 / depth_from_world_scale;

        let colormap = match *properties.color_mapper {
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
        let radius_scale = *properties.backproject_radius_scale;
        let point_radius_from_world_depth = radius_scale * pixel_width_from_depth;

        Ok(DepthCloud {
            world_from_rdf,
            depth_camera_intrinsics: intrinsics.image_from_cam.into(),
            world_depth_from_texture_depth,
            point_radius_from_world_depth,
            max_depth_in_world: world_depth_from_texture_depth * depth_texture.range[1],
            depth_dimensions: dimensions,
            depth_texture: depth_texture.texture,
            colormap,
            outline_mask_id: ent_context.highlight.overall,
            picking_object_id: re_renderer::PickingLayerObjectId(ent_path.hash64()),
        })
    }

    fn extend_bbox(&mut self, textured_rect: &TexturedRect) {
        let left_top = textured_rect.top_left_corner_position;
        let extent_u = textured_rect.extent_u;
        let extent_v = textured_rect.extent_v;
        self.data.bounding_box.extend(left_top);
        self.data.bounding_box.extend(left_top + extent_u);
        self.data.bounding_box.extend(left_top + extent_v);
        self.data
            .bounding_box
            .extend(left_top + extent_v + extent_u);
    }
}

impl NamedViewSystem for ImagesPart {
    fn name() -> re_viewer_context::ViewSystemName {
        "Images".into()
    }
}

impl ViewPartSystem for ImagesPart {
    fn required_components(&self) -> ComponentNameSet {
        let image: ComponentNameSet = Image::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect();
        let depth_image: ComponentNameSet = DepthImage::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect();
        let segmentation_image: ComponentNameSet = SegmentationImage::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect();

        image
            .intersection(&depth_image)
            .map(ToOwned::to_owned)
            .collect::<ComponentNameSet>()
            .intersection(&segmentation_image)
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        [
            Image::indicator_component(),
            DepthImage::indicator_component(),
            SegmentationImage::indicator_component(),
        ]
        .into_iter()
        .collect()
    }

    fn heuristic_filter(
        &self,
        store: &re_arrow_store::DataStore,
        ent_path: &EntityPath,
        entity_components: &ComponentNameSet,
    ) -> bool {
        if !default_heuristic_filter(entity_components, &self.indicator_components()) {
            return false;
        }

        if let Some(tensor) = store.query_latest_component::<TensorData>(
            ent_path,
            &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        ) {
            tensor.is_shaped_like_an_image() && !tensor.is_vector()
        } else {
            false
        }
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut depth_clouds = Vec::new();

        let transforms = view_ctx.get::<TransformContext>()?;

        process_archetype_views::<ImagesPart, Image, { Image::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.image,
            |ctx, ent_path, ent_view, ent_context| {
                let ent_props = query.entity_props_map.get(ent_path);
                self.process_image_arch_view(
                    ctx,
                    transforms,
                    &ent_props,
                    &ent_view,
                    ent_path,
                    ent_context,
                )
            },
        )?;

        process_archetype_views::<
            ImagesPart,
            SegmentationImage,
            { SegmentationImage::NUM_COMPONENTS },
            _,
        >(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.image,
            |ctx, ent_path, ent_view, ent_context| {
                let ent_props = query.entity_props_map.get(ent_path);
                self.process_segmentation_image_arch_view(
                    ctx,
                    transforms,
                    &ent_props,
                    &ent_view,
                    ent_path,
                    ent_context,
                )
            },
        )?;

        process_archetype_views::<ImagesPart, DepthImage, { DepthImage::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.image,
            |ctx, ent_path, ent_view, ent_context| {
                let ent_props = query.entity_props_map.get(ent_path);
                self.process_depth_image_arch_view(
                    ctx,
                    &mut depth_clouds,
                    transforms,
                    &ent_props,
                    &ent_view,
                    ent_path,
                    ent_context,
                )
            },
        )?;

        self.handle_image_layering();

        let mut draw_data_list = Vec::new();

        match re_renderer::renderer::DepthCloudDrawData::new(
            ctx.render_ctx,
            &DepthClouds {
                clouds: depth_clouds,
                radius_boost_in_ui_points_for_outlines: SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
            },
        ) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!(
                    "Failed to create depth cloud draw data from depth images: {err}"
                );
            }
        }
        // TODO(wumpf): Can we avoid this copy, maybe let DrawData take an iterator?
        let rectangles = self
            .images
            .iter()
            .map(|image| image.textured_rect.clone())
            .collect_vec();
        match re_renderer::renderer::RectangleDrawData::new(ctx.render_ctx, &rectangles) {
            Ok(draw_data) => {
                draw_data_list.push(draw_data.into());
            }
            Err(err) => {
                re_log::error_once!("Failed to create rectangle draw data from images: {err}");
            }
        }

        Ok(draw_data_list)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
