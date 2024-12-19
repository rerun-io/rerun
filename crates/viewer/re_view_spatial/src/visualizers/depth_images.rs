use nohash_hasher::IntMap;

use re_entity_db::EntityPath;
use re_log_types::EntityPathHash;
use re_renderer::renderer::{ColormappedTexture, DepthCloud, DepthClouds};
use re_types::{
    archetypes::DepthImage,
    components::{
        self, Colormap, DepthMeter, DrawOrder, FillRatio, ImageBuffer, ImageFormat, ValueRange,
        ViewCoordinates,
    },
    image::ImageKind,
    Component as _,
};
use re_viewer_context::{
    ApplicableEntities, ColormapWithRange, IdentifiedViewSystem, ImageInfo, ImageStatsCache,
    QueryContext, TypedComponentFallbackProvider, ViewClass, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::{SpatialSceneEntityContext, TwoDInThreeDTransformInfo},
    query_pinhole_legacy,
    view_kind::SpatialViewKind,
    visualizers::filter_visualizable_2d_entities,
    PickableRectSourceData, PickableTexturedRect, SpatialView3D,
};

use super::{textured_rect_from_image, SpatialViewVisualizerData};

pub struct DepthImageVisualizer {
    pub data: SpatialViewVisualizerData,

    /// Expose image infos for depth clouds - we need this for picking interaction.
    pub depth_cloud_entities: IntMap<EntityPathHash, (ImageInfo, DepthMeter, ColormappedTexture)>,
}

impl Default for DepthImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
            depth_cloud_entities: IntMap::default(),
        }
    }
}

struct DepthImageComponentData {
    image: ImageInfo,
    depth_meter: Option<DepthMeter>,
    fill_ratio: Option<FillRatio>,
    colormap: Option<Colormap>,
    value_range: Option<[f64; 2]>,
}

impl DepthImageVisualizer {
    fn process_depth_image_data(
        &mut self,
        ctx: &QueryContext<'_>,
        depth_clouds: &mut Vec<DepthCloud>,
        ent_context: &SpatialSceneEntityContext<'_>,
        images: impl Iterator<Item = DepthImageComponentData>,
    ) {
        let is_3d_view = ent_context.view_class_identifier == SpatialView3D::identifier();
        ent_context
            .transform_info
            .warn_on_per_instance_transform(ctx.target_entity_path, "DepthImage");

        let entity_path = ctx.target_entity_path;

        for data in images {
            let DepthImageComponentData {
                image,
                depth_meter,
                fill_ratio,
                colormap,
                value_range,
            } = data;

            let depth_meter = depth_meter.unwrap_or_else(|| self.fallback_for(ctx));

            // All depth images must have a colormap:
            let colormap = colormap.unwrap_or_else(|| self.fallback_for(ctx));
            let value_range = value_range
                .map(|r| [r[0] as f32, r[1] as f32])
                .unwrap_or_else(|| {
                    // Don't use fallback provider since it has to query information we already have.
                    let image_stats = ctx
                        .viewer_ctx
                        .cache
                        .entry(|c: &mut ImageStatsCache| c.entry(&image));
                    ColormapWithRange::default_range_for_depth_images(&image_stats)
                });
            let colormap_with_range = ColormapWithRange {
                colormap,
                value_range,
            };

            // First try to create a textured rect for this image.
            // Even if we end up only showing a depth cloud,
            // we still need most of this for ui interaction which still shows the image!
            let Some(textured_rect) = textured_rect_from_image(
                ctx.viewer_ctx,
                entity_path,
                ent_context,
                &image,
                Some(&colormap_with_range),
                re_renderer::Rgba::WHITE,
                "DepthImage",
                &mut self.data,
            ) else {
                // If we can't create a textured rect from this, we don't have to bother with clouds either.
                return;
            };

            if is_3d_view {
                if let Some(twod_in_threed_info) = &ent_context.transform_info.twod_in_threed_info {
                    let fill_ratio = fill_ratio.unwrap_or_default();

                    // NOTE: we don't pass in `world_from_obj` because this corresponds to the
                    // transform of the projection plane, which is of no use to us here.
                    // What we want are the extrinsics of the depth camera!
                    match Self::process_entity_view_as_depth_cloud(
                        ctx,
                        ent_context,
                        entity_path,
                        twod_in_threed_info,
                        depth_meter,
                        fill_ratio,
                        &textured_rect.colormapped_texture,
                    ) {
                        Ok(cloud) => {
                            self.data.add_bounding_box(
                                entity_path.hash(),
                                cloud.world_space_bbox(),
                                glam::Affine3A::IDENTITY,
                            );
                            self.depth_cloud_entities.insert(
                                entity_path.hash(),
                                (image, depth_meter, textured_rect.colormapped_texture),
                            );
                            depth_clouds.push(cloud);

                            // Skip creating a textured rect.
                            return;
                        }
                        Err(err) => {
                            re_log::warn_once!("{err}");
                        }
                    }
                };
            }

            self.data.pickable_rects.push(PickableTexturedRect {
                ent_path: entity_path.clone(),
                textured_rect,
                source_data: PickableRectSourceData::Image {
                    image,
                    depth_meter: Some(depth_meter),
                },
            });
        }
    }

    fn process_entity_view_as_depth_cloud(
        ctx: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        ent_path: &EntityPath,
        twod_in_threed_info: &TwoDInThreeDTransformInfo,
        depth_meter: DepthMeter,
        radius_scale: FillRatio,
        depth_texture: &ColormappedTexture,
    ) -> anyhow::Result<DepthCloud> {
        re_tracing::profile_function!();

        let Some(intrinsics) = query_pinhole_legacy(
            ctx.viewer_ctx,
            ctx.query,
            &twod_in_threed_info.parent_pinhole,
        ) else {
            anyhow::bail!(
                "Couldn't fetch pinhole intrinsics at {:?}",
                twod_in_threed_info.parent_pinhole
            );
        };

        // Place the cloud at the pinhole's location. Note that this means we ignore any 2D transforms that might be there.
        let world_from_view = twod_in_threed_info.reference_from_pinhole_entity;
        let world_from_rdf = world_from_view
            * glam::Affine3A::from_mat3(
                intrinsics
                    .camera_xyz
                    .unwrap_or(ViewCoordinates::RDF) // TODO(#2641): This should come from archetype
                    .from_rdf(),
            );

        let dimensions = glam::UVec2::from_array(depth_texture.texture.width_height());

        let world_depth_from_texture_depth = 1.0 / *depth_meter.0;

        // We want point radius to be defined in a scale where the radius of a point
        // is a factor of the diameter of a pixel projected at that distance.
        let fov_y = intrinsics.fov_y().unwrap_or(1.0);
        let pixel_width_from_depth = (0.5 * fov_y).tan() / (0.5 * dimensions.y as f32);
        let point_radius_from_world_depth = *radius_scale.0 * pixel_width_from_depth;

        let min_max_depth_in_world = [
            world_depth_from_texture_depth * depth_texture.range[0],
            world_depth_from_texture_depth * depth_texture.range[1],
        ];

        Ok(DepthCloud {
            world_from_rdf,
            depth_camera_intrinsics: intrinsics.image_from_camera.0.into(),
            world_depth_from_texture_depth,
            point_radius_from_world_depth,
            min_max_depth_in_world,
            depth_dimensions: dimensions,
            depth_texture: depth_texture.texture.clone(),
            colormap: match depth_texture.color_mapper {
                re_renderer::renderer::ColorMapper::Function(colormap) => colormap,
                _ => re_renderer::Colormap::Grayscale,
            },
            outline_mask_id: ent_context.highlight.overall,
            picking_object_id: re_renderer::PickingLayerObjectId(ent_path.hash64()),
        })
    }
}

impl IdentifiedViewSystem for DepthImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "DepthImage".into()
    }
}

impl VisualizerSystem for DepthImageVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<DepthImage>()
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
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(ViewSystemExecutionError::NoRenderContextError);
        };

        let mut depth_clouds = Vec::new();

        use super::entity_iterator::{iter_buffer, iter_component, process_archetype};
        process_archetype::<Self, DepthImage, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_buffer_chunks) = results.get_required_chunks(&ImageBuffer::name())
                else {
                    return Ok(());
                };
                let Some(all_format_chunks) = results.get_required_chunks(&ImageFormat::name())
                else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_buffers_indexed =
                    iter_buffer::<u8>(&all_buffer_chunks, timeline, ImageBuffer::name());
                let all_formats_indexed = iter_component::<ImageFormat>(
                    &all_format_chunks,
                    timeline,
                    ImageFormat::name(),
                );
                let all_colormaps = results.iter_as(timeline, Colormap::name());
                let all_value_ranges = results.iter_as(timeline, ValueRange::name());
                let all_depth_meters = results.iter_as(timeline, DepthMeter::name());
                let all_fill_ratios = results.iter_as(timeline, FillRatio::name());

                let mut data = re_query::range_zip_1x5(
                    all_buffers_indexed,
                    all_formats_indexed,
                    all_colormaps.primitive::<u8>(),
                    all_value_ranges.primitive_array::<2, f64>(),
                    all_depth_meters.primitive::<f32>(),
                    all_fill_ratios.primitive::<f32>(),
                )
                .filter_map(
                    |(index, buffers, format, colormap, value_range, depth_meter, fill_ratio)| {
                        let buffer = buffers.first()?;

                        Some(DepthImageComponentData {
                            image: ImageInfo {
                                buffer_row_id: index.1,
                                buffer: buffer.clone().into(),
                                format: first_copied(format.as_deref())?.0,
                                kind: ImageKind::Depth,
                            },
                            depth_meter: first_copied(depth_meter).map(Into::into),
                            fill_ratio: first_copied(fill_ratio).map(Into::into),
                            colormap: first_copied(colormap).and_then(Colormap::from_u8),
                            value_range: first_copied(value_range).map(Into::into),
                        })
                    },
                );

                self.process_depth_image_data(ctx, &mut depth_clouds, spatial_ctx, &mut data);

                Ok(())
            },
        )?;

        let mut draw_data_list = Vec::new();

        match re_renderer::renderer::DepthCloudDrawData::new(
            render_ctx,
            &DepthClouds {
                clouds: depth_clouds,
                radius_boost_in_ui_points_for_outlines:
                    re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
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

        draw_data_list.push(PickableTexturedRect::to_draw_data(
            render_ctx,
            &self.data.pickable_rects,
        )?);

        Ok(draw_data_list)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<DrawOrder> for DepthImageVisualizer {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> DrawOrder {
        DrawOrder::DEFAULT_DEPTH_IMAGE
    }
}

impl TypedComponentFallbackProvider<ValueRange> for DepthImageVisualizer {
    fn fallback_for(
        &self,
        ctx: &re_viewer_context::QueryContext<'_>,
    ) -> re_types::components::ValueRange {
        if let Some(((_time, buffer_row_id), image_buffer)) = ctx
            .recording()
            .latest_at_component::<ImageBuffer>(ctx.target_entity_path, ctx.query)
        {
            // TODO(andreas): What about overrides on the image format?
            if let Some((_, format)) = ctx
                .recording()
                .latest_at_component::<ImageFormat>(ctx.target_entity_path, ctx.query)
            {
                let image = ImageInfo {
                    buffer_row_id,
                    buffer: image_buffer.0,
                    format: format.0,
                    kind: ImageKind::Depth,
                };
                let cache = ctx.viewer_ctx.cache;
                let image_stats = cache.entry(|c: &mut ImageStatsCache| c.entry(&image));
                let default_range = ColormapWithRange::default_range_for_depth_images(&image_stats);
                return [default_range[0] as f64, default_range[1] as f64].into();
            }
        }

        [0.0, f64::MAX].into()
    }
}

impl TypedComponentFallbackProvider<Colormap> for DepthImageVisualizer {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Colormap {
        ColormapWithRange::DEFAULT_DEPTH_COLORMAP
    }
}

impl TypedComponentFallbackProvider<DepthMeter> for DepthImageVisualizer {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> DepthMeter {
        let is_integer_tensor = ctx
            .recording()
            .latest_at_component::<components::TensorData>(ctx.target_entity_path, ctx.query)
            .map_or(false, |(_index, tensor)| tensor.dtype().is_integer());

        if is_integer_tensor { 1000.0 } else { 1.0 }.into()
    }
}

re_viewer_context::impl_component_fallback_provider!(DepthImageVisualizer => [Colormap, ValueRange, DepthMeter, DrawOrder]);

fn first_copied<T: Copy>(slice: Option<&[T]>) -> Option<T> {
    slice.and_then(|element| element.first()).copied()
}
