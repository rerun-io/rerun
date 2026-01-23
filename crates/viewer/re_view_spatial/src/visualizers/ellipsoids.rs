use std::iter;

use re_chunk_store::external::re_chunk::ChunkComponentIterItem;
use re_sdk_types::archetypes::Ellipsoids3D;
use re_sdk_types::components::{ClassId, Color, FillMode, HalfSize3D, Radius, ShowLabels};
use re_sdk_types::{ArrowString, components};
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
pub struct Ellipsoids3DVisualizer(SpatialViewVisualizerData);

impl Default for Ellipsoids3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Ellipsoids3DVisualizer {
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        batches: impl Iterator<Item = Ellipsoids3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        for batch in batches {
            // TODO(kpreid): subdivisions should be configurable, and possibly dynamic based on
            // either world size or screen size (depending on application).
            let subdivisions = match batch.fill_mode {
                FillMode::DenseWireframe => 2, // Don't make it too crowded - let the user see inside the mesh.
                FillMode::Solid => 6,          // Smooth, but not too CPU/GPU intensive
                FillMode::MajorWireframe => 12, // Three smooth ellipses
            };
            let proc_mesh_key = proc_mesh::ProcMeshKey::Sphere {
                subdivisions,
                axes_only: match batch.fill_mode {
                    FillMode::MajorWireframe => true,
                    FillMode::DenseWireframe | FillMode::Solid => false,
                },
            };

            builder.add_batch(
                query_context,
                ent_context,
                Ellipsoids3D::descriptor_colors().component,
                Ellipsoids3D::descriptor_show_labels().component,
                glam::Affine3A::IDENTITY,
                ProcMeshBatch {
                    half_sizes: batch.half_sizes,
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

struct Ellipsoids3DComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSize3D],

    // Clamped to edge
    centers: &'a [components::Translation3D],
    rotation_axis_angles: ChunkComponentIterItem<components::RotationAxisAngle>,
    quaternions: &'a [components::RotationQuat],
    colors: &'a [Color],
    line_radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Ellipsoids3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Ellipsoids3D".into()
    }
}

impl VisualizerSystem for Ellipsoids3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Ellipsoids3D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let preferred_view_kind = self.0.preferred_view_kind;
        let mut output = VisualizerExecutionOutput::default();
        let mut builder = ProcMeshDrawableBuilder::new(
            &mut self.0,
            ctx.viewer_ctx.render_ctx(),
            view_query,
            "ellipsoids",
        );

        use super::entity_iterator::{iter_slices, process_archetype};
        process_archetype::<Self, Ellipsoids3D, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            preferred_view_kind,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let all_half_size_chunks =
                    results.get_required_chunk(Ellipsoids3D::descriptor_half_sizes().component);
                if all_half_size_chunks.is_empty() {
                    return Ok(());
                }

                let num_ellipsoids: usize = all_half_size_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 3]>())
                    .map(|vectors| vectors.len())
                    .sum();
                if num_ellipsoids == 0 {
                    return Ok(());
                }

                // Ideally we would reserve space here, but we don't know the mesh subdivision yet,
                // and this will become moot when we switch to instanced meshes.
                // line_builder.reserve_strips(num_ellipsoids * sphere_mesh.line_strips.len())?;
                // line_builder.reserve_vertices(num_ellipsoids * sphere_mesh.vertex_count)?;

                let timeline = ctx.query.timeline();
                let all_half_sizes_indexed =
                    iter_slices::<[f32; 3]>(&all_half_size_chunks, timeline);
                let all_centers =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_centers().component);
                let all_rotation_axis_angles = results.iter_as(
                    timeline,
                    Ellipsoids3D::descriptor_rotation_axis_angles().component,
                );
                let all_quaternions =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_quaternions().component);
                let all_colors =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_colors().component);
                let all_line_radii =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_line_radii().component);
                let all_fill_modes =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_fill_mode().component);
                let all_labels =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_labels().component);
                let all_class_ids =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_class_ids().component);
                let all_show_labels =
                    results.iter_as(timeline, Ellipsoids3D::descriptor_show_labels().component);

                let data = re_query::range_zip_1x9(
                    all_half_sizes_indexed,
                    all_centers.slice::<[f32; 3]>(),
                    all_rotation_axis_angles.component_slow::<components::RotationAxisAngle>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_colors.slice::<u32>(),
                    all_line_radii.slice::<f32>(),
                    all_fill_modes.slice::<u8>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(
                        _index,
                        half_sizes,
                        centers,
                        rotation_axis_angles,
                        quaternions,
                        colors,
                        line_radii,
                        fill_modes,
                        labels,
                        class_ids,
                        show_labels,
                    )| {
                        Ellipsoids3DComponentData {
                            half_sizes: bytemuck::cast_slice(half_sizes),
                            centers: centers.map_or(&[], bytemuck::cast_slice),
                            rotation_axis_angles: rotation_axis_angles.unwrap_or_default(),
                            quaternions: quaternions.map_or(&[], bytemuck::cast_slice),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            line_radii: line_radii
                                .map_or(&[], |line_radii| bytemuck::cast_slice(line_radii)),
                            // fill mode is currently a non-repeated component
                            fill_mode: fill_modes
                                .unwrap_or_default()
                                .first()
                                .copied()
                                .and_then(FillMode::from_u8)
                                .unwrap_or_default(),
                            labels: labels.unwrap_or_default(),
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
