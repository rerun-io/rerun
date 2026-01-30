use nohash_hasher::IntMap;

use re_entity_db::EntityPath;
use re_log_types::EntityPathHash;
use re_renderer::renderer::{ColormappedTexture, DepthCloud, DepthClouds};
use re_sdk_types::archetypes::DepthImage;
use re_sdk_types::components::{Colormap, DepthMeter, FillRatio, ImageFormat};
use re_sdk_types::{Archetype as _, ArchetypeName, ComponentIdentifier};
use re_viewer_context::{
    ColormapWithRange, IdentifiedViewSystem, ImageInfo, ImageStatsCache, QueryContext,
    ViewClass as _, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
};

use super::entity_iterator::process_archetype;
use super::{SpatialViewVisualizerData, textured_rect_from_image};
use crate::contexts::{SpatialSceneVisualizerInstructionContext, TransformTreeContext};
use crate::view_kind::SpatialViewKind;
use crate::visualizers::first_copied;
use crate::{PickableRectSourceData, PickableTexturedRect, SpatialView3D};

pub struct DepthImageProcessResult {
    pub image_info: ImageInfo,
    pub depth_meter: DepthMeter,
    pub colormap: ColormappedTexture,
}

pub struct DepthImageVisualizer {
    pub data: SpatialViewVisualizerData,

    /// Expose image infos for depth clouds - we need this for picking interaction.
    pub depth_cloud_entities: IntMap<EntityPathHash, DepthImageProcessResult>,
}

impl Default for DepthImageVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
            depth_cloud_entities: IntMap::default(),
        }
    }
}

pub struct DepthImageComponentData {
    pub image: ImageInfo,
    pub depth_meter: Option<DepthMeter>,
    pub fill_ratio: Option<FillRatio>,
    pub colormap: Option<Colormap>,
    pub value_range: Option<[f64; 2]>,
}

#[expect(clippy::too_many_arguments)]
pub fn process_depth_image_data(
    ctx: &QueryContext<'_>,
    ent_context: &mut SpatialSceneVisualizerInstructionContext<'_>,
    data_store: &mut SpatialViewVisualizerData,
    depth_cloud_entities: &mut IntMap<EntityPathHash, DepthImageProcessResult>,
    depth_clouds: &mut Vec<DepthCloud>,
    transforms: &TransformTreeContext,
    component_data: DepthImageComponentData,
    archetype_name: ArchetypeName,
    depth_meter_identifier: ComponentIdentifier,
    colormap_identifier: ComponentIdentifier,
) {
    let is_3d_view = ent_context.view_class_identifier == SpatialView3D::identifier();
    let entity_path = ctx.target_entity_path;

    let DepthImageComponentData {
        image: image_info,
        depth_meter,
        fill_ratio,
        colormap,
        value_range,
    } = component_data;

    let depth_meter =
        depth_meter.unwrap_or_else(|| typed_fallback_for(ctx, depth_meter_identifier));

    // All depth images must have a colormap:
    let colormap = colormap.unwrap_or_else(|| typed_fallback_for(ctx, colormap_identifier));
    let value_range = value_range
        .map(|r| [r[0] as f32, r[1] as f32])
        .unwrap_or_else(|| {
            // Don't use fallback provider since it has to query information we already have.
            let image_stats = ctx
                .store_ctx()
                .caches
                .entry(|c: &mut ImageStatsCache| c.entry(&image_info));
            ColormapWithRange::default_range_for_depth_images(&image_stats)
        });
    let colormap_with_range = ColormapWithRange {
        colormap,
        value_range,
    };

    // First try to create a textured rect for this image.
    // Even if we end up only showing a depth cloud,
    // we still need most of this for ui interaction which still shows the image!
    let textured_rect = match textured_rect_from_image(
        ctx.viewer_ctx(),
        entity_path,
        ent_context,
        &image_info,
        Some(&colormap_with_range),
        re_renderer::Rgba::WHITE,
        archetype_name,
    ) {
        Ok(textured_rect) => textured_rect,
        Err(err) => {
            ent_context.report_error(re_error::format(err));

            // If we can't create a textured rect from this, we don't have to bother with clouds either.
            return;
        }
    };

    if is_3d_view {
        // In 3D views we should show depth images as a depth cloud and no textured rect.
        // For that we need a pinhole at or above that entity in the transform tree.
        let tree_root_frame = ent_context.transform_info.tree_root();
        if let Some(pinhole_tree_root_info) = transforms.pinhole_tree_root_info(tree_root_frame)
            && let Some(world_from_view) = transforms.target_from_pinhole_root(tree_root_frame)
        {
            let fill_ratio = fill_ratio.unwrap_or_default();

            // NOTE: we don't pass in `world_from_obj` because this corresponds to the
            // transform of the projection plane, which is of no use to us here.
            // What we want are the extrinsics of the depth camera!
            let cloud = process_entity_view_as_depth_cloud(
                ent_context,
                entity_path,
                pinhole_tree_root_info,
                world_from_view.as_affine3a(),
                depth_meter,
                fill_ratio,
                &textured_rect.colormapped_texture,
            );
            data_store.add_bounding_box(
                entity_path.hash(),
                cloud.world_space_bbox(),
                glam::Affine3A::IDENTITY,
            );
            depth_cloud_entities.insert(
                entity_path.hash(),
                DepthImageProcessResult {
                    image_info,
                    depth_meter,
                    colormap: textured_rect.colormapped_texture,
                },
            );
            depth_clouds.push(cloud);
        } else {
            ent_context.report_error(
                "Cannot draw depth image as 3D point cloud since it is not under a pinhole camera.",
            );
        }
    } else {
        data_store.add_pickable_rect(
            PickableTexturedRect {
                ent_path: entity_path.clone(),
                textured_rect,
                source_data: PickableRectSourceData::Image {
                    image: image_info,
                    depth_meter: Some(depth_meter),
                },
            },
            ent_context.view_class_identifier,
        );
    }
}

fn process_entity_view_as_depth_cloud(
    ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
    ent_path: &EntityPath,
    pinhole_tree_root_info: &re_tf::PinholeTreeRoot,
    world_from_view: glam::Affine3A,
    depth_meter: DepthMeter,
    radius_scale: FillRatio,
    depth_texture: &ColormappedTexture,
) -> DepthCloud {
    re_tracing::profile_function!();

    // Place the cloud at the pinhole's location. Note that this means we ignore any 2D transforms that might on the way.
    let pinhole = &pinhole_tree_root_info.pinhole_projection;
    let world_from_rdf =
        world_from_view * glam::Affine3A::from_mat3(pinhole.view_coordinates.from_rdf());

    let dimensions = glam::UVec2::from_array(depth_texture.texture.width_height());

    let world_depth_from_texture_depth = 1.0 / *depth_meter.0;

    // We want point radius to be defined in a scale where the radius of a point
    // is a factor of the diameter of a pixel projected at that distance.
    let fov_y = pinhole
        .image_from_camera
        .fov_y(pinhole.resolution.unwrap_or_else(|| [1.0, 1.0].into()));
    let pixel_width_from_depth = (0.5 * fov_y).tan() / (0.5 * dimensions.y as f32);
    let point_radius_from_world_depth = *radius_scale.0 * pixel_width_from_depth;

    let min_max_depth_in_world = [
        world_depth_from_texture_depth * depth_texture.range[0],
        world_depth_from_texture_depth * depth_texture.range[1],
    ];

    DepthCloud {
        world_from_rdf,
        depth_camera_intrinsics: pinhole.image_from_camera.0.into(),
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
    }
}

impl IdentifiedViewSystem for DepthImageVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "DepthImage".into()
    }
}

impl VisualizerSystem for DepthImageVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<DepthImage>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let preferred_view_kind = self.data.preferred_view_kind;

        let mut output = VisualizerExecutionOutput::default();
        let mut depth_clouds = Vec::new();

        let transforms = context_systems.get::<TransformTreeContext>()?;

        process_archetype::<Self, DepthImage, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            preferred_view_kind,
            |ctx, spatial_ctx, results| {
                use super::entity_iterator::{iter_component, iter_slices};
                use re_view::RangeResultsExt as _;

                let all_buffer_chunks = results
                    .get_required_chunk(DepthImage::descriptor_buffer().component)
                    .ensure_required(|err| spatial_ctx.report_error(err));
                if all_buffer_chunks.is_empty() {
                    return Ok(());
                }
                let all_format_chunks = results
                    .get_required_chunk(DepthImage::descriptor_format().component)
                    .ensure_required(|err| spatial_ctx.report_error(err));
                if all_format_chunks.is_empty() {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_buffers_indexed = iter_slices::<&[u8]>(&all_buffer_chunks, timeline);
                let all_formats_indexed =
                    iter_component::<ImageFormat>(&all_format_chunks, timeline);
                let all_colormaps = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    DepthImage::descriptor_colormap().component,
                );
                let all_value_ranges = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    DepthImage::descriptor_depth_range().component,
                );
                let all_depth_meters = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    DepthImage::descriptor_meter().component,
                );
                let all_fill_ratios = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    DepthImage::descriptor_point_fill_ratio().component,
                );

                for (
                    (_time, row_id),
                    buffers,
                    format,
                    colormap,
                    value_range,
                    depth_meter,
                    fill_ratio,
                ) in re_query::range_zip_1x5(
                    all_buffers_indexed,
                    all_formats_indexed,
                    all_colormaps.slice::<u8>(),
                    all_value_ranges.slice::<[f64; 2]>(),
                    all_depth_meters.slice::<f32>(),
                    all_fill_ratios.slice::<f32>(),
                ) {
                    let Some(buffer) = buffers.first() else {
                        spatial_ctx.report_error("Depth image buffer is empty.");
                        continue;
                    };
                    let Some(format) = first_copied(format.as_deref()) else {
                        spatial_ctx.report_error("Depth image format is missing.");
                        continue;
                    };

                    let data = DepthImageComponentData {
                        image: ImageInfo::from_stored_blob(
                            row_id,
                            DepthImage::descriptor_buffer().component,
                            buffer.clone().into(),
                            format.0,
                            re_sdk_types::image::ImageKind::Depth,
                        ),
                        depth_meter: first_copied(depth_meter).map(Into::into),
                        fill_ratio: first_copied(fill_ratio).map(Into::into),
                        colormap: first_copied(colormap).and_then(Colormap::from_u8),
                        value_range: first_copied(value_range),
                    };

                    process_depth_image_data(
                        ctx,
                        spatial_ctx,
                        &mut self.data,
                        &mut self.depth_cloud_entities,
                        &mut depth_clouds,
                        transforms,
                        data,
                        DepthImage::name(),
                        DepthImage::descriptor_meter().component,
                        DepthImage::descriptor_colormap().component,
                    );
                }

                Ok(())
            },
        )?;

        populate_depth_visualizer_execution_result(ctx, &self.data, depth_clouds, output)
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}

pub fn populate_depth_visualizer_execution_result(
    ctx: &ViewContext<'_>,
    data: &SpatialViewVisualizerData,
    depth_clouds: Vec<DepthCloud>,
    mut output: VisualizerExecutionOutput,
) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
    let depth_cloud = re_renderer::renderer::DepthCloudDrawData::new(
        ctx.viewer_ctx.render_ctx(),
        &DepthClouds {
            clouds: depth_clouds,
            radius_boost_in_ui_points_for_outlines:
                re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        },
    )
    .map_err(|err| ViewSystemExecutionError::DrawDataCreationError(Box::new(err)))?;
    output.draw_data.push(depth_cloud.into());
    output.draw_data.push(PickableTexturedRect::to_draw_data(
        ctx.viewer_ctx.render_ctx(),
        &data.pickable_rects,
    )?);

    Ok(output)
}
