use re_chunk_store::RowId;
use re_renderer::external::wgpu;
use re_renderer::renderer::{VolumeDrawData, VolumeOptions};
use re_renderer::resource_managers::Texture3DDataDesc;
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Volume3D;
use re_sdk_types::components::{
    Colormap, GammaCorrection, OpticalDensity, RotationQuat, TensorData, Translation3D, ValueRange,
    VoxelSize,
};
use re_sdk_types::encodings::TensorBuffer;
use re_sdk_types::reflection::Enum as _;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, ViewerReportSeverity, VisualizerExecutionOutput,
    VisualizerQueryInfo, VisualizerSystem, gpu_bridge, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::entity_iterator::process_archetype;
use crate::contexts::SpatialSceneVisualizerInstructionContext;

#[derive(Default)]
pub struct Volume3DVisualizer;

struct Volume3DComponentData {
    row_id: RowId,
    values: TensorData,
    voxel_size: VoxelSize,
    translation: Option<Translation3D>,
    quaternion: Option<RotationQuat>,
    value_range: ValueRange,
    colormap: Colormap,
    gamma: GammaCorrection,
    optical_density: OpticalDensity,
}

impl IdentifiedViewSystem for Volume3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Volume3D".into()
    }
}

impl VisualizerSystem for Volume3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<TensorData>(
            &Volume3D::descriptor_values(),
            &Volume3D::all_components(),
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

        process_archetype::<Volume3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_values = results.iter_required(Volume3D::descriptor_values().component);
                if all_values.is_empty() {
                    return Ok(());
                }

                let all_voxel_sizes =
                    results.iter_optional(Volume3D::descriptor_voxel_size().component);
                let all_translations =
                    results.iter_optional(Volume3D::descriptor_translation().component);
                let all_quaternions =
                    results.iter_optional(Volume3D::descriptor_quaternion().component);
                let all_value_ranges =
                    results.iter_optional(Volume3D::descriptor_value_range().component);
                let all_colormaps =
                    results.iter_optional(Volume3D::descriptor_colormap().component);
                let all_optical_densities =
                    results.iter_optional(Volume3D::descriptor_optical_density().component);

                let all_gammas = results.iter_optional(Volume3D::descriptor_gamma().component);

                let volumes = re_query::range_zip_1x7(
                    all_values.component_slow::<TensorData>(),
                    all_voxel_sizes.slice::<[f32; 3]>(),
                    all_translations.slice::<[f32; 3]>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_value_ranges.slice::<[f64; 2]>(),
                    all_colormaps.slice::<u8>(),
                    all_optical_densities.slice::<f32>(),
                    all_gammas.slice::<f32>(),
                )
                .filter_map(
                    |(
                        (_time, row_id),
                        values,
                        voxel_sizes,
                        translations,
                        quaternions,
                        value_ranges,
                        colormaps,
                        optical_densities,
                        gammas,
                    )| {
                        Some(Volume3DComponentData {
                            row_id,
                            values: values.first()?.clone(),
                            voxel_size: voxel_sizes
                                .and_then(|sizes| sizes.first().copied())
                                .map(VoxelSize::from)
                                .unwrap_or_else(|| {
                                    typed_fallback_for(
                                        ctx,
                                        Volume3D::descriptor_voxel_size().component,
                                    )
                                }),
                            translation: translations
                                .and_then(|values| values.first().copied())
                                .map(Translation3D::from),
                            quaternion: quaternions
                                .and_then(|values| values.first().copied())
                                .map(RotationQuat::from),
                            value_range: value_ranges
                                .and_then(|ranges| ranges.first().copied())
                                .map(ValueRange::from)
                                .unwrap_or_else(|| {
                                    typed_fallback_for(
                                        ctx,
                                        Volume3D::descriptor_value_range().component,
                                    )
                                }),
                            colormap: colormaps
                                .and_then(|values| values.first().copied())
                                .and_then(Colormap::try_from_integer)
                                .unwrap_or_else(|| {
                                    typed_fallback_for(
                                        ctx,
                                        Volume3D::descriptor_colormap().component,
                                    )
                                }),
                            gamma: gammas
                                .and_then(|values| values.first().copied())
                                .map(GammaCorrection::from)
                                .unwrap_or_else(|| {
                                    typed_fallback_for(ctx, Volume3D::descriptor_gamma().component)
                                }),
                            optical_density: optical_densities
                                .and_then(|values| values.first().copied())
                                .map(OpticalDensity::from)
                                .unwrap_or_else(|| {
                                    typed_fallback_for(
                                        ctx,
                                        Volume3D::descriptor_optical_density().component,
                                    )
                                }),
                        })
                    },
                );

                for volume in volumes {
                    if let Some(volume_draw_data) =
                        Self::process_volume(&mut data, ctx, results, spatial_ctx, volume)?
                    {
                        draw_data.push(volume_draw_data.into());
                    }
                }

                Ok(())
            },
        )?;

        Ok(output.with_draw_data(draw_data).with_visualizer_data(data))
    }
}

impl Volume3DVisualizer {
    fn process_volume(
        data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        spatial_ctx: &SpatialSceneVisualizerInstructionContext<'_>,
        component_data: Volume3DComponentData,
    ) -> Result<Option<VolumeDrawData>, ViewSystemExecutionError> {
        let Volume3DComponentData {
            row_id,
            values,
            voxel_size,
            translation,
            quaternion,
            value_range,
            colormap,
            gamma,
            optical_density,
        } = component_data;

        let Some(dimensions) = Self::volume_dimensions(results, &values) else {
            return Ok(None);
        };
        let TensorBuffer::F16(voxels) = &values.buffer else {
            results.report_for_component(
                Volume3D::descriptor_values().component,
                ViewerReportSeverity::Error,
                "Volume3D only supports f16 tensor data",
            );
            return Ok(None);
        };

        let voxel_size = glam::Vec3::from_array(voxel_size.0.0);
        if !voxel_size.is_finite() || !voxel_size.cmpgt(glam::Vec3::ZERO).all() {
            results.report_for_component(
                Volume3D::descriptor_voxel_size().component,
                ViewerReportSeverity::Error,
                "voxel_size must be finite and positive",
            );
            return Ok(None);
        }

        let gamma = gamma.0.0;
        if !gamma.is_finite() || gamma <= 0.0 {
            results.report_for_component(
                Volume3D::descriptor_gamma().component,
                ViewerReportSeverity::Error,
                "gamma must be finite and positive",
            );
            return Ok(None);
        }

        let optical_density = optical_density.0.0;
        if !optical_density.is_finite() || optical_density < 0.0 {
            results.report_for_component(
                Volume3D::descriptor_optical_density().component,
                ViewerReportSeverity::Error,
                "optical_density must be finite and nonnegative",
            );
            return Ok(None);
        }

        let Some(value_range) = Self::validate_value_range(results, value_range) else {
            return Ok(None);
        };
        let Some(entity_from_volume) = Self::entity_from_volume(results, translation, quaternion)
        else {
            return Ok(None);
        };
        let world_from_entity = spatial_ctx
            .transform_info
            .single_transform_required_for_entity(ctx.target_entity_path, Volume3D::name())
            .as_affine3a();
        let world_from_grid = world_from_entity * entity_from_volume;
        let extent = glam::Vec3::from_array(dimensions.map(|value| value as f32)) * voxel_size;
        let world_from_volume = world_from_grid * glam::Affine3A::from_scale(extent);

        let texture_desc = Texture3DDataDesc {
            label: format!("Volume3D {}", ctx.target_entity_path).into(),
            data: bytemuck::cast_slice::<half::f16, u8>(voxels.as_ref()).into(),
            format: wgpu::TextureFormat::R16Float,
            dimensions,
        };
        let render_ctx = ctx.viewer_ctx().render_ctx();
        // One row can in theory contain different tensors; mappings can select different sources within it.
        let texture_key = egui::util::hash((
            row_id,
            results.component_source_hash(Volume3D::descriptor_values().component),
        ));
        let texture = match render_ctx.texture_manager_3d.get_or_create(
            texture_key,
            render_ctx,
            &texture_desc,
        ) {
            Ok(texture) => texture,
            Err(err) => {
                results.report_for_component(
                    Volume3D::descriptor_values().component,
                    ViewerReportSeverity::Error,
                    err.to_string(),
                );
                return Ok(None);
            }
        };

        let draw_data = VolumeDrawData::new(
            render_ctx,
            &texture,
            VolumeOptions {
                world_from_volume,
                value_range,
                colormap: gpu_bridge::colormap_to_re_renderer(colormap),
                gamma,
                optical_density,
                outline_mask_ids: spatial_ctx.highlight.overall,
                picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                    re_entity_db::InstancePathHash::entity_all(ctx.target_entity_path),
                ),
            },
        )?;

        data.add_bounding_box_3d(
            ctx.target_entity_path.hash(),
            macaw::BoundingBox::from_min_max(glam::Vec3::ZERO, glam::Vec3::ONE),
            world_from_volume,
        );

        Ok(Some(draw_data))
    }

    fn volume_dimensions(
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        values: &TensorData,
    ) -> Option<[u32; 3]> {
        let &[depth, height, width] = values.shape() else {
            results.report_for_component(
                Volume3D::descriptor_values().component,
                ViewerReportSeverity::Error,
                "Volume3D values must have exactly three dimensions ordered [z, y, x]",
            );
            return None;
        };
        let [Ok(width), Ok(height), Ok(depth)] = [width, height, depth].map(u32::try_from) else {
            results.report_for_component(
                Volume3D::descriptor_values().component,
                ViewerReportSeverity::Error,
                "Volume3D dimensions exceed the supported u32 range",
            );
            return None;
        };
        let dimensions = [width, height, depth];
        if dimensions.contains(&0) {
            results.report_for_component(
                Volume3D::descriptor_values().component,
                ViewerReportSeverity::Error,
                "Volume3D dimensions must be non-zero",
            );
            return None;
        }

        Some(dimensions)
    }

    fn validate_value_range(
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        value_range: ValueRange,
    ) -> Option<[f32; 2]> {
        let Some(range) = value_range
            .try_as_f32_range()
            .filter(|[min, max]| min < max)
        else {
            results.report_for_component(
                Volume3D::descriptor_value_range().component,
                ViewerReportSeverity::Error,
                "value_range must be finite and increasing",
            );
            return None;
        };

        Some(range)
    }

    fn entity_from_volume(
        results: &re_view::VisualizerInstructionQueryResults<'_>,
        translation: Option<Translation3D>,
        quaternion: Option<RotationQuat>,
    ) -> Option<glam::Affine3A> {
        let translation = translation.map_or(glam::Affine3A::IDENTITY, Into::into);
        let Ok(rotation) = glam::Affine3A::try_from(quaternion.unwrap_or(RotationQuat::IDENTITY))
        else {
            results.report_for_component(
                Volume3D::descriptor_quaternion().component,
                ViewerReportSeverity::Error,
                "invalid rotation quaternion",
            );
            return None;
        };

        Some(translation * rotation)
    }
}
