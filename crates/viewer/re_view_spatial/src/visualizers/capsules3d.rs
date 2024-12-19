use std::iter;

use ordered_float::NotNan;
use re_types::{
    archetypes::Capsules3D,
    components::{self, ClassId, Color, FillMode, HalfSize3D, Length, Radius, ShowLabels, Text},
    ArrowString, Component as _,
};
use re_view::clamped_or_nothing;
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{contexts::SpatialSceneEntityContext, proc_mesh, view_kind::SpatialViewKind};

use super::{
    filter_visualizable_3d_entities,
    utilities::{ProcMeshBatch, ProcMeshDrawableBuilder},
    SpatialViewVisualizerData,
};

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
        builder: &mut ProcMeshDrawableBuilder<'_, Fallback>,
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
                        subdivisions: 4,
                        length: NotNan::new(ratio).unwrap(), // ok because of clean_length()
                    }
                });

            builder.add_batch(
                query_context,
                ent_context,
                glam::Affine3A::IDENTITY,
                ProcMeshBatch {
                    half_sizes: &half_sizes,
                    meshes,
                    // Only Solid is currently supported by proc_mesh
                    fill_modes: iter::repeat(FillMode::Solid),
                    line_radii: &[],
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
    colors: &'a [Color],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Capsules3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Capsules3D".into()
    }
}

impl VisualizerSystem for Capsules3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Capsules3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
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

        let mut builder = ProcMeshDrawableBuilder::new(
            &mut self.0,
            render_ctx,
            view_query,
            "capsules3d",
            &Fallback,
        );

        use super::entity_iterator::{iter_primitive, process_archetype};
        process_archetype::<Self, Capsules3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_length_chunks) = results.get_required_chunks(&Length::name()) else {
                    return Ok(());
                };
                let Some(all_radius_chunks) =
                    results.get_required_chunks(&components::Radius::name())
                else {
                    return Ok(());
                };

                let num_lengths: usize = all_length_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_primitive::<f32>(&Length::name()))
                    .map(|lengths| lengths.len())
                    .sum();
                let num_radii: usize = all_radius_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_primitive::<f32>(&components::Radius::name()))
                    .map(|radii| radii.len())
                    .sum();
                let num_instances = num_lengths.max(num_radii);
                if num_instances == 0 {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_lengths_indexed =
                    iter_primitive::<f32>(&all_length_chunks, timeline, Length::name());
                let all_radii_indexed =
                    iter_primitive::<f32>(&all_radius_chunks, timeline, components::Radius::name());
                let all_colors = results.iter_as(timeline, Color::name());
                let all_labels = results.iter_as(timeline, Text::name());
                let all_show_labels = results.iter_as(timeline, ShowLabels::name());
                let all_class_ids = results.iter_as(timeline, ClassId::name());

                let data = re_query::range_zip_2x4(
                    all_lengths_indexed,
                    all_radii_indexed,
                    all_colors.primitive::<u32>(),
                    all_labels.string(),
                    all_show_labels.bool(),
                    all_class_ids.primitive::<u16>(),
                )
                .map(
                    |(_index, lengths, radii, colors, labels, show_labels, class_ids)| {
                        Capsules3DComponentData {
                            lengths: bytemuck::cast_slice(lengths),
                            radii: bytemuck::cast_slice(radii),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            show_labels: show_labels.unwrap_or_default().get(0).map(Into::into),
                        }
                    },
                );

                Self::process_data(&mut builder, ctx, spatial_ctx, data)?;

                Ok(())
            },
        )?;

        builder.into_draw_data()
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        &Fallback
    }
}

struct Fallback;

impl TypedComponentFallbackProvider<Color> for Fallback {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<ShowLabels> for Fallback {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> ShowLabels {
        super::utilities::show_labels_fallback::<HalfSize3D>(ctx)
    }
}

re_viewer_context::impl_component_fallback_provider!(Fallback => [Color, ShowLabels]);

fn clean_length(suspicious_length: f32) -> f32 {
    if suspicious_length.is_finite() && suspicious_length > 0.0 {
        suspicious_length
    } else {
        // all negatives including negative zero, NaNs, and infinities shall become positive zero
        0.0
    }
}
