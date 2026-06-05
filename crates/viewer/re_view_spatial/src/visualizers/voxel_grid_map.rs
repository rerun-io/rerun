use std::sync::Arc;

use re_renderer::renderer::{VoxelGridDrawData, VoxelGridInstance, VoxelGridOptions};
use re_renderer::{Color32, PickingLayerInstanceId};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::VoxelGridMap;
use re_sdk_types::components::{
    CellSize, Colormap, Opacity, RotationAxisAngle, RotationQuat, Translation3D,
};
use re_sdk_types::datatypes::Quaternion;
use re_sdk_types::reflection::Enum as _;
use re_view::clamped_or_nothing;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerReportSeverity, VisualizerSystem, gpu_bridge, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::entity_iterator::process_archetype;
use crate::contexts::SpatialSceneVisualizerInstructionContext;

const NUM_VOXEL_LIMIT_PER_BATCH: usize = 100_000;

#[derive(Default)]
pub struct VoxelGridMapVisualizer;

#[derive(Clone, Copy)]
struct VoxelGridMapComponentData<'a> {
    indices: &'a [[i32; 3]],
    cell_size: CellSize,
    values: &'a [f32],
    colors: &'a [u32],
    translation: Option<Translation3D>,
    rotation_axis_angle: Option<RotationAxisAngle>,
    quaternion: Option<RotationQuat>,
    opacity: Option<Opacity>,
    value_range: Option<[f64; 2]>,
    colormap: Option<Colormap>,
}

impl IdentifiedViewSystem for VoxelGridMapVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "VoxelGridMap".into()
    }
}

impl VisualizerSystem for VoxelGridMapVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<re_sdk_types::components::VoxelIndex>(
            &VoxelGridMap::descriptor_voxel_indices(),
            &VoxelGridMap::all_components(),
        )
    }

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
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
        let mut draw_data = Vec::new();
        let output = VisualizerExecutionOutput::default();

        process_archetype::<VoxelGridMap, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_indices =
                    results.iter_required(VoxelGridMap::descriptor_voxel_indices().component);
                if all_indices.is_empty() {
                    return Ok(());
                }

                let all_cell_sizes =
                    results.iter_optional(VoxelGridMap::descriptor_cell_size().component);

                let all_values = results.iter_optional(VoxelGridMap::descriptor_values().component);
                let all_colors = results.iter_optional(VoxelGridMap::descriptor_colors().component);
                let all_translations =
                    results.iter_optional(VoxelGridMap::descriptor_translation().component);
                let all_rotations =
                    results.iter_optional(VoxelGridMap::descriptor_rotation_axis_angle().component);
                let all_quaternions =
                    results.iter_optional(VoxelGridMap::descriptor_quaternion().component);
                let all_opacities =
                    results.iter_optional(VoxelGridMap::descriptor_opacity().component);
                let all_value_ranges =
                    results.iter_optional(VoxelGridMap::descriptor_value_range().component);
                let all_colormaps =
                    results.iter_optional(VoxelGridMap::descriptor_colormap().component);

                let voxel_maps = re_query::range_zip_1x9(
                    all_indices.slice::<[i32; 3]>(),
                    all_cell_sizes.slice::<f32>(),
                    all_values.slice::<f32>(),
                    all_colors.slice::<u32>(),
                    all_translations.slice::<[f32; 3]>(),
                    all_rotations.component_slow::<RotationAxisAngle>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_opacities.slice::<f32>(),
                    all_value_ranges.slice::<[f64; 2]>(),
                    all_colormaps.slice::<u8>(),
                )
                .map(
                    |(
                        _index,
                        indices,
                        cell_sizes,
                        values,
                        colors,
                        translations,
                        rotations,
                        quaternions,
                        opacities,
                        value_ranges,
                        colormaps,
                    )| {
                        VoxelGridMapComponentData {
                            indices,
                            cell_size: cell_sizes
                                .and_then(|cell_sizes| cell_sizes.first().copied())
                                .map(CellSize::from)
                                .unwrap_or_else(|| {
                                    typed_fallback_for(
                                        ctx,
                                        VoxelGridMap::descriptor_cell_size().component,
                                    )
                                }),
                            values: values.unwrap_or(&[]),
                            colors: colors.unwrap_or(&[]),
                            translation: translations
                                .and_then(|t| t.first().copied())
                                .map(Translation3D::from),
                            rotation_axis_angle: rotations.and_then(|r| r.first().copied()),
                            quaternion: quaternions
                                .and_then(|q| q.first().copied())
                                .map(RotationQuat::from),
                            opacity: opacities.and_then(|o| o.first().copied()).map(Into::into),
                            value_range: value_ranges.and_then(|r| r.first().copied()),
                            colormap: colormaps
                                .and_then(|c| c.first().copied())
                                .and_then(Colormap::try_from_integer),
                        }
                    },
                );

                for voxel_map in voxel_maps {
                    if let Some(voxel_draw_data) = Self::process_voxel_grid_map(
                        &mut data,
                        ctx,
                        results,
                        spatial_ctx,
                        &output,
                        voxel_map,
                    )? {
                        draw_data.push(voxel_draw_data.into());
                    }
                }

                Ok(())
            },
        )?;

        Ok(output.with_draw_data(draw_data).with_visualizer_data(data))
    }
}

impl VoxelGridMapVisualizer {
    fn process_voxel_grid_map(
        data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        spatial_ctx: &SpatialSceneVisualizerInstructionContext<'_>,
        output: &VisualizerExecutionOutput,
        component_data: VoxelGridMapComponentData<'_>,
    ) -> Result<Option<VoxelGridDrawData>, ViewSystemExecutionError> {
        let entity_path = ctx.target_entity_path;
        let VoxelGridMapComponentData {
            indices,
            cell_size,
            values,
            colors,
            translation,
            rotation_axis_angle,
            quaternion,
            opacity,
            value_range,
            colormap,
        } = component_data;

        if indices.is_empty() {
            return Ok(None);
        }

        let cell_size = f32::from(cell_size.0);
        if !(cell_size.is_finite() && cell_size > 0.0) {
            results.report_for_component(
                VoxelGridMap::descriptor_cell_size().component,
                VisualizerReportSeverity::Error,
                "cell_size must be positive",
            );
            return Ok(None);
        }

        let Some(world_from_grid) = Self::world_from_grid(
            ctx,
            results,
            spatial_ctx,
            translation,
            rotation_axis_angle,
            quaternion,
        ) else {
            return Ok(None);
        };

        let max_voxels = if ctx.app_ctx().app_options.visualizer_limits_enabled
            && indices.len() > NUM_VOXEL_LIMIT_PER_BATCH
        {
            if let Some(instruction_id) = ctx.instruction_id {
                output.report_unspecified_source(
                    instruction_id,
                    VisualizerReportSeverity::Warning,
                    format!(
                        "Too many voxels ({}), capping to {}. This limit can be lifted in Settings.",
                        re_format::format_uint(indices.len()),
                        re_format::format_uint(NUM_VOXEL_LIMIT_PER_BATCH),
                    ),
                );
            }
            NUM_VOXEL_LIMIT_PER_BATCH
        } else {
            indices.len()
        };

        let opacity = opacity
            .unwrap_or_else(|| {
                typed_fallback_for(ctx, VoxelGridMap::descriptor_opacity().component)
            })
            .0
            .clamp(0.0, 1.0);

        let colors = Self::resolve_colors(ctx, max_voxels, colors, values, value_range, colormap);

        let mut voxel_instances = Vec::with_capacity(max_voxels);
        let mut local_bbox = macaw::BoundingBox::nothing();
        for (instance_index, (index, mut color)) in std::iter::zip(indices, colors).enumerate() {
            if instance_index >= max_voxels {
                break;
            }

            let alpha = ((color.a() as f32) * opacity).round().clamp(0.0, 255.0) as u8;
            if alpha == 0 {
                continue;
            }
            #[expect(clippy::disallowed_methods)]
            // This alpha comes from logged data, not from a hard-coded UI color.
            {
                color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
            }

            let index = glam::IVec3::from_array(*index);
            voxel_instances.push(VoxelGridInstance {
                index,
                color,
                picking_instance_id: PickingLayerInstanceId(instance_index as _),
            });

            let min = index.as_vec3() * cell_size;
            let max = (index + glam::IVec3::ONE).as_vec3() * cell_size;
            local_bbox = local_bbox.union(macaw::BoundingBox::from_min_max(min, max));
        }

        if voxel_instances.is_empty() {
            return Ok(None);
        }

        let world_bbox = local_bbox.transform_affine3(&world_from_grid);
        data.add_bounding_box(entity_path.hash(), world_bbox, glam::Affine3A::IDENTITY);

        let draw_data = VoxelGridDrawData::new(
            ctx.viewer_ctx().render_ctx(),
            &voxel_instances,
            VoxelGridOptions {
                world_from_grid,
                draw_order_position: world_bbox.center().into(),
                cell_size,
                opacity: 1.0,
                picking_object_id: re_renderer::PickingLayerObjectId(entity_path.hash64()),
                outline_mask_ids: spatial_ctx.highlight.overall,
                depth_offset: spatial_ctx.depth_offset,
            },
        )
        .map_err(|err| ViewSystemExecutionError::DrawDataCreationError(Arc::new(err)))?;

        Ok(Some(draw_data))
    }

    fn resolve_colors(
        ctx: &QueryContext<'_>,
        num_voxels: usize,
        colors: &[u32],
        values: &[f32],
        value_range: Option<[f64; 2]>,
        colormap: Option<Colormap>,
    ) -> Vec<Color32> {
        if !colors.is_empty() {
            return clamped_or_nothing(colors, num_voxels)
                .map(|&color| Color32::from(re_sdk_types::components::Color::from(color)))
                .collect();
        }

        if !values.is_empty() {
            let value_range = value_range
                .map(|[min, max]| [min as f32, max as f32])
                .unwrap_or_else(|| {
                    let range: re_sdk_types::components::ValueRange =
                        typed_fallback_for(ctx, VoxelGridMap::descriptor_value_range().component);
                    [range.0.0[0] as f32, range.0.0[1] as f32]
                });
            let colormap = colormap.unwrap_or_else(|| {
                typed_fallback_for(ctx, VoxelGridMap::descriptor_colormap().component)
            });
            let colormap = gpu_bridge::colormap_to_re_renderer(colormap);
            let value_span = value_range[1] - value_range[0];

            return clamped_or_nothing(values, num_voxels)
                .map(|&value| {
                    let t = if value_span.is_finite() && value_span > 0.0 {
                        (value - value_range[0]) / value_span
                    } else {
                        0.5
                    };
                    let [r, g, b, a] = re_renderer::colormap_srgba(colormap, t);
                    #[expect(clippy::disallowed_methods)]
                    // This color comes from the selected data colormap, not from a hard-coded UI color.
                    {
                        Color32::from_rgba_unmultiplied(r, g, b, a)
                    }
                })
                .collect();
        }

        let fallback_color: re_sdk_types::components::Color =
            typed_fallback_for(ctx, VoxelGridMap::descriptor_colors().component);
        vec![Color32::from(fallback_color); num_voxels]
    }

    fn world_from_grid(
        ctx: &QueryContext<'_>,
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        spatial_ctx: &SpatialSceneVisualizerInstructionContext<'_>,
        translation: Option<Translation3D>,
        rotation_axis_angle: Option<RotationAxisAngle>,
        quaternion: Option<RotationQuat>,
    ) -> Option<glam::Affine3A> {
        let entity_path = ctx.target_entity_path;
        let world_from_entity = spatial_ctx
            .transform_info
            .single_transform_required_for_entity(entity_path, VoxelGridMap::name())
            .as_affine3a();

        let translation = translation.map_or(glam::Affine3A::IDENTITY, Into::into);

        let rotation = match (quaternion, rotation_axis_angle) {
            (Some(quaternion), Some(rotation_axis_angle))
                if quaternion.0 != Quaternion::IDENTITY
                    && rotation_axis_angle != RotationAxisAngle::IDENTITY =>
            {
                results.report_for_component(
                    VoxelGridMap::descriptor_quaternion().component,
                    VisualizerReportSeverity::Warning,
                    format!(
                        "VoxelGridMap {entity_path} has both quaternion and rotation_axis_angle set; using quaternion."
                    ),
                );

                let Ok(rotation) = glam::Affine3A::try_from(quaternion) else {
                    results.report_for_component(
                        VoxelGridMap::descriptor_quaternion().component,
                        VisualizerReportSeverity::Error,
                        "invalid rotation quaternion",
                    );
                    return None;
                };
                rotation
            }
            (Some(quaternion), _) => {
                let Ok(rotation) = glam::Affine3A::try_from(quaternion) else {
                    results.report_for_component(
                        VoxelGridMap::descriptor_quaternion().component,
                        VisualizerReportSeverity::Error,
                        "invalid rotation quaternion",
                    );
                    return None;
                };
                rotation
            }
            (_, Some(rotation_axis_angle)) => {
                let Ok(rotation) = glam::Affine3A::try_from(rotation_axis_angle) else {
                    results.report_for_component(
                        VoxelGridMap::descriptor_rotation_axis_angle().component,
                        VisualizerReportSeverity::Error,
                        "invalid rotation axis-angle",
                    );
                    return None;
                };
                rotation
            }
            (None, None) => glam::Affine3A::IDENTITY,
        };

        Some(world_from_entity * translation * rotation)
    }
}
