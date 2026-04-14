use glam::Vec3;
use re_renderer::renderer;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::GridMap;
use re_sdk_types::components::{
    CellSize, Colormap, ImageBuffer, ImageFormat, Opacity, RotationAxisAngle, RotationQuat,
    Translation3D,
};
use re_sdk_types::datatypes::{ColorModel, Quaternion};
use re_sdk_types::image::ImageKind;
use re_sdk_types::reflection::Enum as _;
use re_viewer_context::{
    ColormapWithRange, IdentifiedViewSystem, ImageInfo, QueryContext, ViewClass as _, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput,
    VisualizerQueryInfo, VisualizerReportSeverity, VisualizerSystem, gpu_bridge,
    typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::entity_iterator::process_archetype;
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::{PickableRectSourceData, PickableTexturedRect};

#[derive(Default)]
pub struct GridMapVisualizer;

enum GridMapColorMode {
    NoColormap,
    Colormapped(ColormapWithRange),
}

impl GridMapColorMode {
    fn colormap(&self) -> Option<&re_viewer_context::ColormapWithRange> {
        match self {
            Self::NoColormap => None,
            Self::Colormapped(colormap) => Some(colormap),
        }
    }
}

struct GridMapComponentData {
    image: ImageInfo,
    cell_size: CellSize,
    translation: Option<Translation3D>,
    rotation_axis_angle: Option<RotationAxisAngle>,
    quaternion: Option<RotationQuat>,
    opacity: Option<Opacity>,
    colormap: Option<Colormap>,
}

impl IdentifiedViewSystem for GridMapVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "GridMap".into()
    }
}

impl VisualizerSystem for GridMapVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::buffer_and_format::<ImageBuffer, ImageFormat>(
            &GridMap::descriptor_data(),
            &GridMap::descriptor_format(),
            &GridMap::all_components(),
        )
    }

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        // Prefer 3D view.
        // Grid maps are 2D, but most commonly used as planar layers within a 3D context.
        Some(crate::SpatialView3D::identifier())
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let mut data = SpatialViewVisualizerData::default();
        let output = VisualizerExecutionOutput::default();

        process_archetype::<GridMap, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                Self::process_grid_map(&mut data, ctx, results, spatial_ctx);
                Ok(())
            },
        )?;

        Ok(output
            .with_draw_data([PickableTexturedRect::to_draw_data(
                ctx.viewer_ctx.render_ctx(),
                &data.pickable_rects,
            )?])
            .with_visualizer_data(data))
    }
}

impl GridMapVisualizer {
    fn process_grid_map(
        data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        spatial_ctx: &SpatialSceneVisualizerInstructionContext<'_>,
    ) {
        re_tracing::profile_function!();

        let entity_path = ctx.target_entity_path;

        let all_buffers = results.iter_required(GridMap::descriptor_data().component);
        if all_buffers.is_empty() {
            return;
        }
        let all_formats = results.iter_required(GridMap::descriptor_format().component);
        if all_formats.is_empty() {
            return;
        }
        let all_cell_sizes = results.iter_required(GridMap::descriptor_cell_size().component);
        if all_cell_sizes.is_empty() {
            return;
        }
        let all_translations = results.iter_optional(GridMap::descriptor_translation().component);
        let all_rotations =
            results.iter_optional(GridMap::descriptor_rotation_axis_angle().component);
        let all_quaternions = results.iter_optional(GridMap::descriptor_quaternion().component);
        let all_colormaps = results.iter_optional(GridMap::descriptor_colormap().component);
        let all_opacities = results.iter_optional(GridMap::descriptor_opacity().component);

        let grid_maps = re_query::range_zip_1x7(
            all_buffers.slice::<&[u8]>(),
            all_formats.component_slow::<ImageFormat>(),
            all_cell_sizes.slice::<f32>(),
            all_translations.slice::<[f32; 3]>(),
            all_rotations.component_slow::<RotationAxisAngle>(),
            all_quaternions.slice::<[f32; 4]>(),
            all_opacities.slice::<f32>(),
            all_colormaps.slice::<u8>(),
        )
        .filter_map(
            |(
                (_time, row_id),
                buffers,
                formats,
                cell_sizes,
                translations,
                rotations,
                quaternions,
                opacities,
                colormaps,
            )| {
                let buffer = buffers.first()?;

                Some(GridMapComponentData {
                    image: ImageInfo::from_stored_blob(
                        row_id,
                        GridMap::descriptor_data().component,
                        buffer.clone().into(),
                        formats?.first()?.0,
                        ImageKind::Color,
                    ),
                    cell_size: CellSize::from(*cell_sizes?.first()?),
                    translation: translations
                        .and_then(|t| t.first().copied())
                        .map(Translation3D::from),
                    rotation_axis_angle: rotations.and_then(|r| r.first().copied()),
                    quaternion: quaternions
                        .and_then(|q| q.first().copied())
                        .map(RotationQuat::from),
                    opacity: opacities.and_then(|o| o.first().copied()).map(Into::into),
                    colormap: colormaps
                        .and_then(|c| c.first().copied())
                        .and_then(Colormap::try_from_integer),
                })
            },
        );

        for component_data in grid_maps {
            let color_mode = Self::color_mode_for_grid_map(ctx, results, &component_data);
            if let Some((textured_rect, image)) = Self::textured_rect_from_grid_map(
                ctx,
                results,
                entity_path,
                spatial_ctx,
                component_data,
                &color_mode,
            ) {
                data.add_bounding_box(
                    entity_path.hash(),
                    textured_rect.bounding_box(),
                    glam::Affine3A::IDENTITY,
                );
                data.add_pickable_rect(
                    PickableTexturedRect {
                        ent_path: entity_path.clone(),
                        textured_rect,
                        source_data: PickableRectSourceData::Image {
                            image,
                            depth_meter: None,
                        },
                    },
                    spatial_ctx.view_class_identifier,
                );
            }
        }
    }

    fn textured_rect_from_grid_map(
        ctx: &QueryContext<'_>,
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        entity_path: &re_log_types::EntityPath,
        spatial_ctx: &SpatialSceneVisualizerInstructionContext<'_>,
        component_data: GridMapComponentData,
        color_mode: &GridMapColorMode,
    ) -> Option<(renderer::TexturedRect, ImageInfo)> {
        let GridMapComponentData {
            image,
            cell_size,
            translation,
            rotation_axis_angle,
            quaternion,
            opacity,
            colormap: _,
        } = component_data;

        let cell_size = f32::from(cell_size.0);
        if !(cell_size.is_finite() && cell_size > 0.0) {
            results.report_for_component(
                GridMap::descriptor_cell_size().component,
                VisualizerReportSeverity::Error,
                "cell_size must be positive",
            );
            return None;
        }

        let image_stats = ctx
            .viewer_ctx()
            .store_context
            .caches
            .memoizer(|c: &mut re_viewer_context::ImageStatsCache| c.entry(&image));

        let colormapped_texture = match gpu_bridge::image_to_gpu(
            ctx.viewer_ctx().render_ctx(),
            &entity_path.to_string(),
            &image,
            &image_stats,
            Some(&spatial_ctx.annotations),
            color_mode.colormap(),
        ) {
            Ok(texture) => texture,
            Err(err) => {
                results.report_for_component(
                    GridMap::descriptor_data().component,
                    VisualizerReportSeverity::Error,
                    re_error::format(err),
                );
                return None;
            }
        };

        let world_from_entity = spatial_ctx
            .transform_info
            .single_transform_required_for_entity(entity_path, GridMap::name())
            .as_affine3a();

        let translation = if let Some(translation) = translation {
            translation.into()
        } else {
            glam::Affine3A::IDENTITY
        };

        let rotation = match (quaternion, rotation_axis_angle) {
            (Some(quaternion), Some(rotation_axis_angle))
                if quaternion.0 != Quaternion::IDENTITY
                    && rotation_axis_angle != RotationAxisAngle::IDENTITY =>
            {
                // Match the behavior documented in the archetype definition:
                // if both are set, the quaternion takes precedence.
                results.report_for_component(
                    GridMap::descriptor_quaternion().component,
                    VisualizerReportSeverity::Warning,
                    format!(
                        "GridMap {entity_path} has both quaternion and rotation_axis_angle set; using quaternion."
                    ),
                );

                if let Ok(rotation) = glam::Affine3A::try_from(quaternion) {
                    rotation
                } else {
                    results.report_for_component(
                        GridMap::descriptor_quaternion().component,
                        VisualizerReportSeverity::Error,
                        "invalid rotation quaternion",
                    );
                    return None;
                }
            }
            (Some(quaternion), _) => {
                if let Ok(rotation) = glam::Affine3A::try_from(quaternion) {
                    rotation
                } else {
                    results.report_for_component(
                        GridMap::descriptor_quaternion().component,
                        VisualizerReportSeverity::Error,
                        "invalid rotation quaternion",
                    );
                    return None;
                }
            }
            (_, Some(rotation_axis_angle)) => {
                if let Ok(rotation) = glam::Affine3A::try_from(rotation_axis_angle) {
                    rotation
                } else {
                    results.report_for_component(
                        GridMap::descriptor_rotation_axis_angle().component,
                        VisualizerReportSeverity::Error,
                        "invalid rotation axis-angle",
                    );
                    return None;
                }
            }
            (None, None) => glam::Affine3A::IDENTITY,
        };

        let grid_from_entity = translation * rotation;
        let world_from_grid = world_from_entity * grid_from_entity;

        let [width, height] = image.width_height_f32();
        let extent_u = world_from_grid.transform_vector3(Vec3::X * width * cell_size);
        let extent_v = world_from_grid.transform_vector3(Vec3::NEG_Y * height * cell_size);
        let top_left_corner_position =
            world_from_grid.transform_point3(Vec3::new(0.0, height * cell_size, 0.0));
        let opacity = opacity
            .unwrap_or_else(|| typed_fallback_for(ctx, GridMap::descriptor_opacity().component));
        #[expect(clippy::disallowed_methods)]
        // Lint against hard-coded UI colors doesn't apply here.
        let multiplicative_tint = re_renderer::Rgba::from_white_alpha(opacity.0.clamp(0.0, 1.0));

        let textured_rect = renderer::TexturedRect {
            top_left_corner_position,
            extent_u,
            extent_v,
            colormapped_texture,
            options: renderer::RectangleOptions {
                texture_filter_magnification: renderer::TextureFilterMag::Nearest,
                texture_filter_minification: renderer::TextureFilterMin::Linear,
                multiplicative_tint,
                depth_offset: spatial_ctx.depth_offset,
                outline_mask: spatial_ctx.highlight.overall,
            },
        };

        Some((textured_rect, image))
    }

    fn color_mode_for_grid_map(
        ctx: &QueryContext<'_>,
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        component_data: &GridMapComponentData,
    ) -> GridMapColorMode {
        let Some(colormap) = component_data.colormap else {
            return GridMapColorMode::NoColormap;
        };

        if component_data.image.format.color_model() != ColorModel::L {
            results.report_for_component(
                GridMap::descriptor_colormap().component,
                VisualizerReportSeverity::Warning,
                format!(
                    "GridMap colormaps currently only apply to single-channel maps; ignoring colormap for {:?} data.",
                    component_data.image.format.color_model()
                ),
            );
            return GridMapColorMode::NoColormap;
        }

        let image_stats =
            ctx.viewer_ctx().store_context.caches.memoizer(
                |c: &mut re_viewer_context::ImageStatsCache| c.entry(&component_data.image),
            );

        // TODO(michael): add support for RViz "Map"/"Costmap" colormaps
        // and use [0.0, 255.0] as fixed range for them, if they are selected.
        let value_range = {
            // For conventional colormaps, use the image data range.
            let range =
                gpu_bridge::image_data_range_heuristic(&image_stats, &component_data.image.format);
            [range.min, range.max]
        };

        GridMapColorMode::Colormapped(ColormapWithRange {
            colormap,
            value_range,
        })
    }
}
