use std::iter;

use ordered_float::NotNan;
use re_chunk_store::external::re_chunk::ChunkComponentIterItem;
use re_sdk_types::archetypes::Capsules3D;
use re_sdk_types::components::{ClassId, Color, FillMode, HalfSize3D, Length, Radius, ShowLabels};
use re_sdk_types::{ArrowString, components};
use re_view::clamped_or_nothing;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use super::utilities::{ProcMeshBatch, ProcMeshDrawableBuilder};
use crate::contexts::SpatialSceneEntityContext;
use crate::proc_mesh;
use crate::view_kind::SpatialViewKind;

// ---
pub struct Capsules3DVisualizer(SpatialViewVisualizerData);

impl Default for Capsules3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Capsules3DVisualizer {
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        batches: impl Iterator<Item = Capsules3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        for batch in batches {
            // Number of instances is determined by whichever is *longer* of `lengths` and `radii`.
            // The other component is clamped (last value repeated) to match.
            let num_instances = batch.radii.len().max(batch.lengths.len());
            let lengths_iter = clamped_or_nothing(batch.lengths, num_instances);
            let radii_iter = clamped_or_nothing(batch.radii, num_instances);

            let half_sizes: Vec<HalfSize3D> = radii_iter
                .clone()
                .map(|&Radius(radius)| HalfSize3D::splat(clean_length(radius.0)))
                .collect();

            let subdivisions = match batch.fill_mode {
                FillMode::DenseWireframe => 3, // Don't make it too crowded - let the user see inside the mesh.
                FillMode::Solid => 4,          // Smooth, but not too CPU/GPU intensive
                FillMode::MajorWireframe => 10,
            };

            let axes_only = match batch.fill_mode {
                FillMode::MajorWireframe => true,
                FillMode::DenseWireframe | FillMode::Solid => false,
            };

            let meshes = lengths_iter
                .zip(radii_iter)
                .map(|(&Length(length), &Radius(radius))| {
                    let ratio = clean_length(length.0 / radius.0);

                    // Avoid generating extremely similar meshes by rounding the ratio.
                    // Note: This will cause jitter in the displayed length of the capsule.
                    // TODO(kpreid): Replace this entirely with stretchable meshes.
                    let granularity = 2.0f32.powi(-4); // a round number in base 2
                    let ratio = (ratio / granularity).round() * granularity;

                    proc_mesh::ProcMeshKey::Capsule {
                        subdivisions,
                        length: NotNan::new(ratio)
                            .expect("Ratio was not properly checked with clean_length"),
                        axes_only,
                    }
                });

            builder.add_batch(
                query_context,
                ent_context,
                Capsules3D::descriptor_colors().component,
                Capsules3D::descriptor_show_labels().component,
                glam::Affine3A::IDENTITY,
                ProcMeshBatch {
                    half_sizes: &half_sizes,
                    centers: batch.translations,
                    rotation_axis_angles: batch.rotation_axis_angles.as_slice(),
                    quaternions: batch.quaternions,
                    meshes,
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

struct Capsules3DComponentData<'a> {
    // Point of views
    lengths: &'a [Length],
    radii: &'a [Radius],

    // Clamped to edge
    translations: &'a [components::Translation3D],
    rotation_axis_angles: ChunkComponentIterItem<components::RotationAxisAngle>,
    quaternions: &'a [components::RotationQuat],
    colors: &'a [Color],
    labels: Vec<ArrowString>,
    line_radii: &'a [Radius],
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Capsules3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Capsules3D".into()
    }
}

impl VisualizerSystem for Capsules3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Capsules3D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();
        let preferred_view_kind = self.0.preferred_view_kind;
        let mut builder = ProcMeshDrawableBuilder::new(
            &mut self.0,
            ctx.viewer_ctx.render_ctx(),
            view_query,
            "capsules3d",
        );

        use super::entity_iterator::{iter_slices, process_archetype};
        process_archetype::<Self, Capsules3D, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            preferred_view_kind,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_length_chunks) =
                    results.get_required_chunks(Capsules3D::descriptor_lengths().component)
                else {
                    return Ok(());
                };
                let Some(all_radius_chunks) =
                    results.get_required_chunks(Capsules3D::descriptor_radii().component)
                else {
                    return Ok(());
                };

                let num_lengths: usize = all_length_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<f32>())
                    .map(|lengths| lengths.len())
                    .sum();
                let num_radii: usize = all_radius_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<f32>())
                    .map(|radii| radii.len())
                    .sum();
                let num_instances = num_lengths.max(num_radii);
                if num_instances == 0 {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_lengths_indexed = iter_slices::<f32>(&all_length_chunks, timeline);
                let all_radii_indexed = iter_slices::<f32>(&all_radius_chunks, timeline);
                let all_translations =
                    results.iter_as(timeline, Capsules3D::descriptor_translations().component);
                let all_rotation_axis_angles = results.iter_as(
                    timeline,
                    Capsules3D::descriptor_rotation_axis_angles().component,
                );
                let all_quaternions =
                    results.iter_as(timeline, Capsules3D::descriptor_quaternions().component);
                let all_colors =
                    results.iter_as(timeline, Capsules3D::descriptor_colors().component);
                let all_labels =
                    results.iter_as(timeline, Capsules3D::descriptor_labels().component);
                let all_show_labels =
                    results.iter_as(timeline, Capsules3D::descriptor_show_labels().component);
                let all_fill_modes =
                    results.iter_as(timeline, Capsules3D::descriptor_fill_mode().component);
                let all_line_radii =
                    results.iter_as(timeline, Capsules3D::descriptor_line_radii().component);
                let all_class_ids =
                    results.iter_as(timeline, Capsules3D::descriptor_class_ids().component);

                let data = re_query::range_zip_2x9(
                    all_lengths_indexed,
                    all_radii_indexed,
                    all_translations.slice::<[f32; 3]>(),
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
                        translations,
                        rotation_axis_angles,
                        quaternions,
                        colors,
                        line_radii,
                        fill_modes,
                        labels,
                        show_labels,
                        class_ids,
                    )| {
                        Capsules3DComponentData {
                            lengths: bytemuck::cast_slice(lengths),
                            radii: bytemuck::cast_slice(radii),
                            translations: translations.map_or(&[], bytemuck::cast_slice),
                            rotation_axis_angles: rotation_axis_angles.unwrap_or_default(),
                            quaternions: quaternions.map_or(&[], bytemuck::cast_slice),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            labels: labels.unwrap_or_default(),
                            line_radii: line_radii
                                .map_or(&[], |line_radii| bytemuck::cast_slice(line_radii)),
                            fill_mode: fill_modes
                                .unwrap_or_default()
                                .first()
                                .copied()
                                .and_then(FillMode::from_u8)
                                .unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
                        }
                    },
                );

                Self::process_data(&mut builder, ctx, spatial_ctx, data)?;

                Ok(())
            },
        )?;

        Ok(output.with_draw_data(builder.into_draw_data()?))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }
}

fn clean_length(suspicious_length: f32) -> f32 {
    if suspicious_length.is_finite() && suspicious_length > 0.0 {
        suspicious_length
    } else {
        // all negatives including negative zero, NaNs, and infinities shall become positive zero
        0.0
    }
}
