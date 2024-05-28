use std::collections::BTreeMap;

use egui::NumExt;
use itertools::Itertools as _;
use nohash_hasher::IntSet;

use re_entity_db::{EntityPath, EntityProperties};
use re_log_types::{EntityPathHash, RowId, TimeInt};
use re_query::range_zip_1x2;
use re_renderer::{
    renderer::{DepthCloud, DepthClouds, RectangleOptions, TexturedRect},
    Colormap,
};
use re_space_view::diff_component_filter;
use re_types::{
    archetypes::{DepthImage, Image, SegmentationImage},
    components::{Color, DrawOrder, TensorData, ViewCoordinates},
    tensor_data::{DecodedTensor, TensorDataMeaning},
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    gpu_bridge, ApplicableEntities, DefaultColor, IdentifiedViewSystem, SpaceViewClass,
    SpaceViewSystemExecutionError, TensorDecodeCache, TensorStatsCache, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizableEntities, VisualizableFilterContext,
    VisualizerAdditionalApplicabilityFilter, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext, TransformContext},
    query_pinhole,
    view_kind::SpatialSpaceViewKind,
    visualizers::{filter_visualizable_2d_entities, SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES},
    SpatialSpaceView2D, SpatialSpaceView3D,
};

use super::SpatialViewVisualizerData;

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
    ctx: &ViewerContext<'_>,
    ent_path: &EntityPath,
    ent_context: &SpatialSceneEntityContext<'_>,
    tensor_data_row_id: RowId,
    tensor: &DecodedTensor,
    meaning: TensorDataMeaning,
    multiplicative_tint: egui::Rgba,
) -> Option<re_renderer::renderer::TexturedRect> {
    let [height, width, _] = tensor.image_height_width_channels()?;

    let debug_name = ent_path.to_string();
    let tensor_stats = ctx
        .cache
        .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));

    match gpu_bridge::tensor_to_gpu(
        ctx.render_ctx,
        &debug_name,
        tensor_data_row_id,
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
            let texture_filter_minification = if colormapped_texture.color_mapper.is_on() {
                re_renderer::renderer::TextureFilterMin::Nearest
            } else {
                re_renderer::renderer::TextureFilterMin::Linear
            };

            Some(re_renderer::renderer::TexturedRect {
                top_left_corner_position: ent_context
                    .world_from_entity
                    .transform_point3(glam::Vec3::ZERO),
                extent_u: ent_context
                    .world_from_entity
                    .transform_vector3(glam::Vec3::X * width as f32),
                extent_v: ent_context
                    .world_from_entity
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
            re_log::error_once!("Failed to create texture for {debug_name:?}: {err}");
            None
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ImageGrouping {
    parent_pinhole: Option<EntityPathHash>,
    draw_order: DrawOrder,
}

pub struct ImageVisualizer {
    pub data: SpatialViewVisualizerData,
    pub images: Vec<ViewerImage>,
    pub depth_cloud_entities: IntSet<EntityPathHash>,
}

impl Default for ImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
            images: Vec::new(),
            depth_cloud_entities: IntSet::default(),
        }
    }
}

struct ImageComponentData<'a> {
    index: (TimeInt, RowId),

    tensor: &'a TensorData,
    color: Option<&'a Color>,
    draw_order: Option<&'a DrawOrder>,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl ImageVisualizer {
    fn handle_image_layering(&mut self) {
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
    fn process_image_data<'a>(
        &mut self,
        ctx: &ViewerContext<'_>,
        transforms: &TransformContext,
        ent_props: &EntityProperties,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = ImageComponentData<'a>>,
    ) {
        // If this isn't an image, return
        // TODO(jleibs): The ArchetypeView should probably do this for us.
        if !ctx.recording_store().entity_has_component(
            &ctx.current_query().timeline(),
            entity_path,
            &Image::indicator().name(),
        ) {
            return;
        }

        // Parent pinhole should only be relevant to 3D views
        let parent_pinhole_path =
            if ent_context.space_view_class_identifier == SpatialSpaceView3D::identifier() {
                transforms.parent_pinhole(entity_path)
            } else {
                None
            };

        // Unknown is currently interpreted as "Some Color" in most cases.
        // TODO(jleibs): Make this more explicit
        let meaning = TensorDataMeaning::Unknown;

        for data in data {
            if !data.tensor.is_shaped_like_an_image() {
                continue;
            }

            let tensor_data_row_id = data.index.1;
            let tensor = match ctx.cache.entry(|c: &mut TensorDecodeCache| {
                c.entry(tensor_data_row_id, data.tensor.0.clone())
            }) {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {entity_path}: {err}"
                    );
                    continue;
                }
            };

            let color = ent_context
                .annotations
                .resolved_class_description(None)
                .annotation_info()
                .color(data.color.map(|c| c.to_array()), DefaultColor::OpaqueWhite);

            if let Some(textured_rect) = to_textured_rect(
                ctx,
                entity_path,
                ent_context,
                tensor_data_row_id,
                &tensor,
                meaning,
                color.into(),
            ) {
                // Only update the bounding box if this is a 2D space view or
                // the image_plane_distance is not auto. This is avoids a cyclic
                // relationship where the image plane grows the bounds which in
                // turn influence the size of the image plane.
                // See: https://github.com/rerun-io/rerun/issues/3728
                if ent_context.space_view_class_identifier == SpatialSpaceView2D::identifier()
                    || !ent_props.pinhole_image_plane_distance.is_auto()
                {
                    self.data.add_bounding_box(
                        entity_path.hash(),
                        Self::compute_bounding_box(&textured_rect),
                        ent_context.world_from_entity,
                    );
                }

                self.images.push(ViewerImage {
                    ent_path: entity_path.clone(),
                    tensor,
                    meaning,
                    textured_rect,
                    parent_pinhole: parent_pinhole_path.map(|p| p.hash()),
                    draw_order: data.draw_order.copied().unwrap_or(DrawOrder::DEFAULT_IMAGE),
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_segmentation_image_data<'a>(
        &mut self,
        ctx: &ViewerContext<'_>,
        transforms: &TransformContext,
        ent_props: &EntityProperties,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = ImageComponentData<'a>>,
    ) {
        // If this isn't an image, return
        // TODO(jleibs): The ArchetypeView should probably to this for us.
        if !ctx.recording_store().entity_has_component(
            &ctx.current_query().timeline(),
            entity_path,
            &SegmentationImage::indicator().name(),
        ) {
            return;
        }

        // Parent pinhole should only be relevant to 3D views
        let parent_pinhole_path =
            if ent_context.space_view_class_identifier == SpatialSpaceView3D::identifier() {
                transforms.parent_pinhole(entity_path)
            } else {
                None
            };

        let meaning = TensorDataMeaning::ClassId;

        for data in data {
            if !data.tensor.is_shaped_like_an_image() {
                continue;
            }

            let tensor_data_row_id = data.index.1;
            let tensor = match ctx.cache.entry(|c: &mut TensorDecodeCache| {
                c.entry(tensor_data_row_id, data.tensor.0.clone())
            }) {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {entity_path}: {err}"
                    );
                    continue;
                }
            };

            let color = ent_context
                .annotations
                .resolved_class_description(None)
                .annotation_info()
                .color(data.color.map(|c| c.to_array()), DefaultColor::OpaqueWhite);

            if let Some(textured_rect) = to_textured_rect(
                ctx,
                entity_path,
                ent_context,
                tensor_data_row_id,
                &tensor,
                meaning,
                color.into(),
            ) {
                // Only update the bounding box if this is a 2D space view or
                // the image_plane_distance is not auto. This is avoids a cyclic
                // relationship where the image plane grows the bounds which in
                // turn influence the size of the image plane.
                // See: https://github.com/rerun-io/rerun/issues/3728
                if ent_context.space_view_class_identifier == SpatialSpaceView2D::identifier()
                    || !ent_props.pinhole_image_plane_distance.is_auto()
                {
                    self.data.add_bounding_box(
                        entity_path.hash(),
                        Self::compute_bounding_box(&textured_rect),
                        ent_context.world_from_entity,
                    );
                }

                self.images.push(ViewerImage {
                    ent_path: entity_path.clone(),
                    tensor,
                    meaning,
                    textured_rect,
                    parent_pinhole: parent_pinhole_path.map(|p| p.hash()),
                    draw_order: data.draw_order.copied().unwrap_or(DrawOrder::DEFAULT_IMAGE),
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_depth_image_data<'a>(
        &mut self,
        ctx: &ViewerContext<'_>,
        depth_clouds: &mut Vec<DepthCloud>,
        transforms: &TransformContext,
        ent_props: &EntityProperties,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = ImageComponentData<'a>>,
    ) {
        // If this isn't an image, return
        // TODO(jleibs): The ArchetypeView should probably to this for us.
        if !ctx.recording_store().entity_has_component(
            &ctx.current_query().timeline(),
            entity_path,
            &DepthImage::indicator().name(),
        ) {
            return;
        }

        // Parent pinhole should only be relevant to 3D views
        let parent_pinhole_path =
            if ent_context.space_view_class_identifier == SpatialSpaceView3D::identifier() {
                transforms.parent_pinhole(entity_path)
            } else {
                None
            };

        let meaning = TensorDataMeaning::Depth;

        for data in data {
            // NOTE: we ignore the `DepthMeter` component here because we get it from
            // `EntityProperties::depth_from_world_scale` instead, which is initialized to the
            // same value, but the user may have edited it.

            if !data.tensor.is_shaped_like_an_image() {
                continue;
            }

            let tensor_data_row_id = data.index.1;
            let tensor = match ctx.cache.entry(|c: &mut TensorDecodeCache| {
                c.entry(tensor_data_row_id, data.tensor.0.clone())
            }) {
                Ok(tensor) => tensor,
                Err(err) => {
                    re_log::warn_once!(
                        "Encountered problem decoding tensor at path {entity_path}: {err}"
                    );
                    continue;
                }
            };

            if *ent_props.backproject_depth {
                if let Some(parent_pinhole_path) = transforms.parent_pinhole(entity_path) {
                    // NOTE: we don't pass in `world_from_obj` because this corresponds to the
                    // transform of the projection plane, which is of no use to us here.
                    // What we want are the extrinsics of the depth camera!
                    match Self::process_entity_view_as_depth_cloud(
                        ctx,
                        transforms,
                        ent_context,
                        ent_props,
                        tensor_data_row_id,
                        &tensor,
                        entity_path,
                        parent_pinhole_path,
                    ) {
                        Ok(cloud) => {
                            self.data.add_bounding_box(
                                entity_path.hash(),
                                cloud.world_space_bbox(),
                                glam::Affine3A::IDENTITY,
                            );
                            self.depth_cloud_entities.insert(entity_path.hash());
                            depth_clouds.push(cloud);
                            return;
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
                .color(data.color.map(|c| c.to_array()), DefaultColor::OpaqueWhite);

            if let Some(textured_rect) = to_textured_rect(
                ctx,
                entity_path,
                ent_context,
                tensor_data_row_id,
                &tensor,
                meaning,
                color.into(),
            ) {
                // Only update the bounding box if this is a 2D space view or
                // the image_plane_distance is not auto. This is avoids a cyclic
                // relationship where the image plane grows the bounds which in
                // turn influence the size of the image plane.
                // See: https://github.com/rerun-io/rerun/issues/3728
                if ent_context.space_view_class_identifier == SpatialSpaceView2D::identifier()
                    || !ent_props.pinhole_image_plane_distance.is_auto()
                {
                    self.data.add_bounding_box(
                        entity_path.hash(),
                        Self::compute_bounding_box(&textured_rect),
                        ent_context.world_from_entity,
                    );
                }

                self.images.push(ViewerImage {
                    ent_path: entity_path.clone(),
                    tensor,
                    meaning,
                    textured_rect,
                    parent_pinhole: parent_pinhole_path.map(|p| p.hash()),
                    draw_order: data.draw_order.copied().unwrap_or(DrawOrder::DEFAULT_IMAGE),
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view_as_depth_cloud(
        ctx: &ViewerContext<'_>,
        transforms: &TransformContext,
        ent_context: &SpatialSceneEntityContext<'_>,
        properties: &EntityProperties,
        tensor_data_row_id: RowId,
        tensor: &DecodedTensor,
        ent_path: &EntityPath,
        parent_pinhole_path: &EntityPath,
    ) -> anyhow::Result<DepthCloud> {
        re_tracing::profile_function!();

        let Some(intrinsics) =
            query_pinhole(ctx.recording(), &ctx.current_query(), parent_pinhole_path)
        else {
            anyhow::bail!("Couldn't fetch pinhole intrinsics at {parent_pinhole_path:?}");
        };

        // Place the cloud at the pinhole's location. Note that this means we ignore any 2D transforms that might be there.
        let world_from_view = transforms.reference_from_entity_ignoring_pinhole(
            parent_pinhole_path,
            ctx.recording(),
            &ctx.current_query(),
        );
        let Some(world_from_view) = world_from_view else {
            anyhow::bail!("Couldn't fetch pinhole extrinsics at {parent_pinhole_path:?}");
        };
        let world_from_rdf = world_from_view
            * glam::Affine3A::from_mat3(
                intrinsics
                    .camera_xyz
                    .unwrap_or(ViewCoordinates::RDF) // TODO(#2641): This should come from archetype
                    .from_rdf(),
            );

        let Some([height, width, _]) = tensor.image_height_width_channels() else {
            anyhow::bail!("Tensor at {ent_path:?} is not an image");
        };
        let dimensions = glam::UVec2::new(width as _, height as _);

        let debug_name = ent_path.to_string();
        let tensor_stats = ctx
            .cache
            .entry(|c: &mut TensorStatsCache| c.entry(tensor_data_row_id, tensor));
        let depth_texture = re_viewer_context::gpu_bridge::depth_tensor_to_gpu(
            ctx.render_ctx,
            &debug_name,
            tensor_data_row_id,
            tensor,
            &tensor_stats,
        )?;

        let depth_from_world_scale = *properties.depth_from_world_scale;

        let world_depth_from_texture_depth = 1.0 / depth_from_world_scale;

        let colormap = match *properties.color_mapper {
            re_entity_db::ColorMapper::Colormap(colormap) => match colormap {
                re_entity_db::Colormap::Grayscale => Colormap::Grayscale,
                re_entity_db::Colormap::Turbo => Colormap::Turbo,
                re_entity_db::Colormap::Viridis => Colormap::Viridis,
                re_entity_db::Colormap::Plasma => Colormap::Plasma,
                re_entity_db::Colormap::Magma => Colormap::Magma,
                re_entity_db::Colormap::Inferno => Colormap::Inferno,
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
            depth_camera_intrinsics: intrinsics.image_from_camera.0.into(),
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

    fn compute_bounding_box(textured_rect: &TexturedRect) -> macaw::BoundingBox {
        let left_top = textured_rect.top_left_corner_position;
        let extent_u = textured_rect.extent_u;
        let extent_v = textured_rect.extent_v;

        macaw::BoundingBox::from_points(
            [
                left_top,
                left_top + extent_u,
                left_top + extent_v,
                left_top + extent_v + extent_u,
            ]
            .into_iter(),
        )
    }
}

impl IdentifiedViewSystem for ImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Images".into()
    }
}

struct ImageVisualizerEntityFilter;

impl VisualizerAdditionalApplicabilityFilter for ImageVisualizerEntityFilter {
    fn update_applicability(&mut self, event: &re_data_store::StoreEvent) -> bool {
        diff_component_filter(event, |tensor: &re_types::components::TensorData| {
            tensor.is_shaped_like_an_image()
        })
    }
}

impl VisualizerSystem for ImageVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let indicators = [
            Image::indicator().name(),
            DepthImage::indicator().name(),
            SegmentationImage::indicator().name(),
        ]
        .into_iter()
        .collect();

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

        let required = image
            .intersection(&depth_image)
            .map(ToOwned::to_owned)
            .collect::<ComponentNameSet>()
            .intersection(&segmentation_image)
            .map(ToOwned::to_owned)
            .collect();

        let queried = Image::all_components()
            .iter()
            .chain(DepthImage::all_components().iter())
            .chain(SegmentationImage::all_components().iter())
            .map(ToOwned::to_owned)
            .collect();

        VisualizerQueryInfo {
            indicators,
            required,
            queried,
        }
    }

    fn applicability_filter(&self) -> Option<Box<dyn VisualizerAdditionalApplicabilityFilter>> {
        Some(Box::new(ImageVisualizerEntityFilter))
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_2d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        view_query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut depth_clouds = Vec::new();

        self.process_image_archetype::<Image, _>(
            ctx,
            view_query,
            view_ctx,
            &mut depth_clouds,
            |visualizer,
             ctx,
             _depth_clouds,
             transforms,
             entity_props,
             entity_path,
             spatial_ctx,
             data| {
                visualizer.process_image_data(
                    ctx,
                    transforms,
                    entity_props,
                    entity_path,
                    spatial_ctx,
                    data,
                );
            },
        )?;

        self.process_image_archetype::<SegmentationImage, _>(
            ctx,
            view_query,
            view_ctx,
            &mut depth_clouds,
            |visualizer,
             ctx,
             _depth_clouds,
             transforms,
             entity_props,
             entity_path,
             spatial_ctx,
             data| {
                visualizer.process_segmentation_image_data(
                    ctx,
                    transforms,
                    entity_props,
                    entity_path,
                    spatial_ctx,
                    data,
                );
            },
        )?;

        self.process_image_archetype::<DepthImage, _>(
            ctx,
            view_query,
            view_ctx,
            &mut depth_clouds,
            |visualizer,
             ctx,
             depth_clouds,
             transforms,
             entity_props,
             entity_path,
             spatial_ctx,
             data| {
                visualizer.process_depth_image_data(
                    ctx,
                    depth_clouds,
                    transforms,
                    entity_props,
                    entity_path,
                    spatial_ctx,
                    data,
                );
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

impl ImageVisualizer {
    fn process_image_archetype<A: Archetype, F>(
        &mut self,
        ctx: &ViewerContext<'_>,
        view_query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
        depth_clouds: &mut Vec<DepthCloud>,
        mut f: F,
    ) -> Result<(), SpaceViewSystemExecutionError>
    where
        F: FnMut(
            &mut Self,
            &ViewerContext<'_>,
            &mut Vec<DepthCloud>,
            &TransformContext,
            &EntityProperties,
            &EntityPath,
            &SpatialSceneEntityContext<'_>,
            &mut dyn Iterator<Item = ImageComponentData<'_>>,
        ),
    {
        let transforms = view_ctx.get::<TransformContext>()?;

        super::entity_iterator::process_archetype::<Self, A, _>(
            ctx,
            view_query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.image,
            |ctx, entity_path, entity_props, spatial_ctx, results| {
                re_tracing::profile_scope!(format!("{entity_path}"));

                use crate::visualizers::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let tensors = match results.get_dense::<TensorData>(resolver) {
                    Some(tensors) => tensors?,
                    _ => return Ok(()),
                };

                let colors = results.get_or_empty_dense(resolver)?;
                let draw_orders = results.get_or_empty_dense(resolver)?;

                let mut data = range_zip_1x2(
                    tensors.range_indexed(),
                    draw_orders.range_indexed(),
                    colors.range_indexed(),
                )
                .filter_map(|(&index, tensors, draw_orders, colors)| {
                    tensors.first().map(|tensor| ImageComponentData {
                        index,
                        tensor,
                        color: colors.and_then(|colors| colors.first()),
                        draw_order: draw_orders.and_then(|draw_orders| draw_orders.first()),
                    })
                });

                f(
                    self,
                    ctx,
                    depth_clouds,
                    transforms,
                    entity_props,
                    entity_path,
                    spatial_ctx,
                    &mut data,
                );

                Ok(())
            },
        )?;

        Ok(())
    }
}
