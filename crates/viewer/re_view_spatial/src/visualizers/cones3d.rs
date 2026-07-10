use std::iter;

use re_chunk_store::external::re_chunk::ChunkComponentIterItem;
use re_sdk_types::archetypes::Cones3D;
use re_sdk_types::components::{ClassId, Color, FillMode, HalfSize3D, Length, Radius, ShowLabels};
use re_sdk_types::reflection::Enum as _;
use re_sdk_types::{Archetype as _, ArrowString, components};
use re_view::clamped_or_else;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::utilities::{ProcMeshBatch, ProcMeshDrawableBuilder};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::proc_mesh;

// ---
#[derive(Default)]
pub struct Cones3DVisualizer;

impl Cones3DVisualizer {
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        batches: impl Iterator<Item = Cones3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        for batch in batches {
            let num_instances = batch.radii.len().max(batch.lengths.len());
            let lengths_iter = clamped_or_else(batch.lengths, || {
                typed_fallback_for::<Length>(query_context, Cones3D::descriptor_lengths().component)
            })
            .take(num_instances);
            let radii_iter = clamped_or_else(batch.radii, || {
                typed_fallback_for::<Radius>(query_context, Cones3D::descriptor_radii().component)
            })
            .take(num_instances);

            let half_sizes: Vec<HalfSize3D> = std::iter::zip(lengths_iter, radii_iter)
                .map(|(Length(length), Radius(radius))| {
                    let radius = clean_length(radius.0);
                    HalfSize3D::new(radius, radius, length.0 / 2.0)
                })
                .collect();

            let subdivisions = match batch.fill_mode {
                FillMode::DenseWireframe => 3,
                FillMode::Solid | FillMode::TransparentFillMajorWireframe => 4,
                FillMode::MajorWireframe => 10,
            };

            let proc_mesh_key = proc_mesh::ProcMeshKey::Cone {
                subdivisions,
                axes_only: batch.fill_mode.axes_only(),
            };

            builder.add_batch(
                query_context,
                ent_context,
                Cones3D::descriptor_colors().component,
                Cones3D::descriptor_line_radii().component,
                Cones3D::descriptor_show_labels().component,
                glam::Affine3A::IDENTITY,
                ProcMeshBatch {
                    half_sizes: &half_sizes,
                    centers: batch.centers,
                    rotation_axis_angles: batch.rotation_axis_angles.as_slice(),
                    quaternions: batch.quaternions,
                    meshes: iter::repeat(proc_mesh_key),
                    fill_modes: iter::repeat(batch.fill_mode),
                    line_radii: batch.line_radii,
                    colors: batch.colors,
                    labels: &batch.labels,
                    show_labels: batch.show_labels,
                    class_ids: batch.class_ids,
                },
            )?;
        }

        Ok(())
    }
}

// ---

struct Cones3DComponentData<'a> {
    lengths: &'a [Length],
    radii: &'a [Radius],
    centers: &'a [components::Translation3D],
    rotation_axis_angles: ChunkComponentIterItem<components::RotationAxisAngle>,
    quaternions: &'a [components::RotationQuat],
    colors: &'a [Color],
    labels: Vec<ArrowString>,
    line_radii: &'a [Radius],
    class_ids: &'a [ClassId],
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Cones3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Cones3D"
        )
    }
}

impl VisualizerSystem for Cones3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Length>(
            &Cones3D::descriptor_lengths(),
            &Cones3D::all_components(),
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
        let output = VisualizerExecutionOutput::default();
        let mut data = SpatialViewVisualizerData::default();
        let mut builder = ProcMeshDrawableBuilder::new(
            &mut data,
            ctx.viewer_ctx.render_ctx(),
            view_query,
            &output,
            "cones3d",
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Cones3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_lengths = results.iter_required(Cones3D::descriptor_lengths().component);
                let all_radii = results.iter_optional(Cones3D::descriptor_radii().component);
                let all_centers = results.iter_optional(Cones3D::descriptor_centers().component);
                let all_rotation_axis_angles =
                    results.iter_optional(Cones3D::descriptor_rotation_axis_angles().component);
                let all_quaternions =
                    results.iter_optional(Cones3D::descriptor_quaternions().component);
                let all_colors = results.iter_optional(Cones3D::descriptor_colors().component);
                let all_labels = results.iter_optional(Cones3D::descriptor_labels().component);
                let all_fill_modes =
                    results.iter_optional(Cones3D::descriptor_fill_mode().component);
                let all_line_radii =
                    results.iter_optional(Cones3D::descriptor_line_radii().component);
                let all_show_labels =
                    results.iter_optional(Cones3D::descriptor_show_labels().component);
                let all_class_ids =
                    results.iter_optional(Cones3D::descriptor_class_ids().component);

                let data = re_query::range_zip_1x10(
                    all_lengths.slice::<f32>(),
                    all_radii.slice::<f32>(),
                    all_centers.slice::<[f32; 3]>(),
                    all_rotation_axis_angles.component_slow::<components::RotationAxisAngle>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_colors.slice::<u32>(),
                    all_line_radii.slice::<f32>(),
                    all_fill_modes.slice::<u8>(),
                    all_labels.slice::<String>(),
                    all_show_labels.slice::<bool>(),
                    all_class_ids.slice::<u16>(),
                )
                .map(
                    |(
                        _index,
                        lengths,
                        radii,
                        centers,
                        rotation_axis_angles,
                        quaternions,
                        colors,
                        line_radii,
                        fill_modes,
                        labels,
                        show_labels,
                        class_ids,
                    )| Cones3DComponentData {
                        lengths: bytemuck::cast_slice(lengths),
                        radii: radii.map_or(&[], bytemuck::cast_slice),
                        centers: centers.map_or(&[], bytemuck::cast_slice),
                        rotation_axis_angles: rotation_axis_angles.unwrap_or_default(),
                        quaternions: quaternions.map_or(&[], bytemuck::cast_slice),
                        colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                        labels: labels.unwrap_or_default(),
                        line_radii: line_radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                        fill_mode: fill_modes
                            .and_then(|s| FillMode::from_integer_slice(s).next()?)
                            .unwrap_or_default(),
                        class_ids: class_ids
                            .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                        show_labels: show_labels
                            .map(|b| !b.is_empty() && b.value(0))
                            .map(Into::into),
                    },
                );

                Self::process_data(&mut builder, ctx, spatial_ctx, data)?;

                Ok(())
            },
        )?;

        let draw_data = builder.into_draw_data()?;
        Ok(output.with_draw_data(draw_data).with_visualizer_data(data))
    }
}

fn clean_length(suspicious_length: f32) -> f32 {
    if suspicious_length.is_finite() && suspicious_length > 0.0 {
        suspicious_length
    } else {
        0.0
    }
}
