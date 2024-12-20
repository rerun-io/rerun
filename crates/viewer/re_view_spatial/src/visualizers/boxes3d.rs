use std::iter;

use re_types::{
    archetypes::Boxes3D,
    components::{ClassId, Color, FillMode, HalfSize3D, Radius, ShowLabels, Text},
    ArrowString, Component as _,
};
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

pub struct Boxes3DVisualizer(SpatialViewVisualizerData);

impl Default for Boxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Boxes3DVisualizer {
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_, Fallback>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        batches: impl Iterator<Item = Boxes3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        let proc_mesh_key = proc_mesh::ProcMeshKey::Cube;

        // `ProcMeshKey::Cube` is scaled to side length of 1, i.e. a half-size of 0.5.
        // Therefore, to make scaling by half_size work out to the correct result,
        // apply a factor of 2.
        // TODO(kpreid): Is there any non-historical reason to keep that.
        let constant_instance_transform = glam::Affine3A::from_scale(glam::Vec3::splat(2.0));

        for batch in batches {
            builder.add_batch(
                query_context,
                ent_context,
                constant_instance_transform,
                ProcMeshBatch {
                    half_sizes: batch.half_sizes,
                    meshes: iter::repeat(proc_mesh_key),
                    fill_modes: iter::repeat(batch.fill_mode),
                    line_radii: batch.radii,
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

struct Boxes3DComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSize3D],

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Boxes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes3D".into()
    }
}

impl VisualizerSystem for Boxes3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Boxes3D>()
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

        let mut builder =
            ProcMeshDrawableBuilder::new(&mut self.0, render_ctx, view_query, "boxes3d", &Fallback);

        use super::entity_iterator::{iter_primitive_array, process_archetype};
        process_archetype::<Self, Boxes3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_half_size_chunks) = results.get_required_chunks(&HalfSize3D::name())
                else {
                    return Ok(());
                };

                let num_boxes: usize = all_half_size_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_primitive_array::<3, f32>(&HalfSize3D::name()))
                    .map(|vectors| vectors.len())
                    .sum();
                if num_boxes == 0 {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_half_sizes_indexed = iter_primitive_array::<3, f32>(
                    &all_half_size_chunks,
                    timeline,
                    HalfSize3D::name(),
                );
                let all_colors = results.iter_as(timeline, Color::name());
                let all_radii = results.iter_as(timeline, Radius::name());
                let all_labels = results.iter_as(timeline, Text::name());
                let all_class_ids = results.iter_as(timeline, ClassId::name());
                let all_show_labels = results.iter_as(timeline, ShowLabels::name());

                // Deserialized because it's a union.
                let all_fill_modes = results.iter_as(timeline, FillMode::name());
                // fill mode is currently a non-repeated component
                let fill_mode: FillMode = all_fill_modes
                    .primitive::<u8>()
                    .next()
                    .and_then(|(_, fill_modes)| {
                        fill_modes.first().copied().and_then(FillMode::from_u8)
                    })
                    .unwrap_or_default();

                match fill_mode {
                    FillMode::DenseWireframe | FillMode::MajorWireframe => {
                        // Each box consists of 4 strips with a total of 16 vertices
                        builder.line_builder.reserve_strips(num_boxes * 4)?;
                        builder.line_builder.reserve_vertices(num_boxes * 16)?;
                    }
                    FillMode::Solid => {
                        // No lines.
                    }
                }

                let data = re_query::range_zip_1x5(
                    all_half_sizes_indexed,
                    all_colors.primitive::<u32>(),
                    all_radii.primitive::<f32>(),
                    all_labels.string(),
                    all_class_ids.primitive::<u16>(),
                    all_show_labels.bool(),
                )
                .map(
                    |(_index, half_sizes, colors, radii, labels, class_ids, show_labels)| {
                        Boxes3DComponentData {
                            half_sizes: bytemuck::cast_slice(half_sizes),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            // fill mode is currently a non-repeated component
                            fill_mode,
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
