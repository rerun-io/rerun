use itertools::Itertools as _;

use re_types::{
    archetypes,
    components::{Color, MarkerShape, MarkerSize, Name, Scalar, SeriesVisible},
    Archetype as _, Component as _,
};
use re_view::{clamped_or_nothing, range_with_blueprint_resolved_data};
use re_viewer_context::{
    auto_color_for_entity_path, external::re_entity_db::InstancePath, IdentifiedViewSystem,
    QueryContext, TypedComponentFallbackProvider, ViewContext, ViewQuery, ViewStateExt as _,
    ViewSystemExecutionError, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    series_query::{
        all_scalars_indices, allocate_plot_points, collect_colors, collect_radius_ui,
        collect_scalars, collect_series_name, collect_series_visibility, determine_num_series,
    },
    util::{determine_time_per_pixel, determine_time_range, points_to_series},
    view_class::TimeSeriesViewState,
    PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, ScatterAttrs,
};

/// The system for rendering [`archetypes::SeriesPoints`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesPointSystem {
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesPointSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesPoints".into()
    }
}

// We use a larger default stroke width for scatter plots so the marker is
// visible.
const DEFAULT_MARKER_SIZE: f32 = 3.0;

impl VisualizerSystem for SeriesPointSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<archetypes::Scalars>();
        query_info.queried.extend(
            archetypes::SeriesPoints::all_components()
                .iter()
                .map(|descr| descr.component_name),
        );

        query_info.indicators =
            [archetypes::SeriesPoints::descriptor_indicator().component_name].into();

        query_info
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.load_scalars(ctx, query);
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Color> for SeriesPointSystem {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<MarkerSize> for SeriesPointSystem {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> MarkerSize {
        MarkerSize::from(DEFAULT_MARKER_SIZE)
    }
}

impl TypedComponentFallbackProvider<Name> for SeriesPointSystem {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Name {
        let state = ctx.view_state.downcast_ref::<TimeSeriesViewState>();

        state
            .ok()
            .and_then(|state| {
                state
                    .default_names_for_entities
                    .get(ctx.target_entity_path)
                    .map(|name| name.clone().into())
            })
            .or_else(|| {
                ctx.target_entity_path
                    .last()
                    .map(|part| part.ui_string().into())
            })
            .unwrap_or_default()
    }
}

impl TypedComponentFallbackProvider<SeriesVisible> for SeriesPointSystem {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> SeriesVisible {
        true.into()
    }
}

re_viewer_context::impl_component_fallback_provider!(SeriesPointSystem => [Color, MarkerSize, Name, SeriesVisible]);

impl SeriesPointSystem {
    fn load_scalars(&mut self, ctx: &ViewContext<'_>, query: &ViewQuery<'_>) {
        re_tracing::profile_function!();

        let plot_mem =
            egui_plot::PlotMemory::load(ctx.viewer_ctx.egui_ctx(), crate::plot_id(query.view_id));
        let time_per_pixel = determine_time_per_pixel(ctx.viewer_ctx, plot_mem.as_ref());

        let data_results = query.iter_visible_data_results(Self::identifier());

        let parallel_loading = true;
        if parallel_loading {
            use rayon::prelude::*;
            re_tracing::profile_wait!("load_series");
            for mut one_series in data_results
                .collect_vec()
                .par_iter()
                .map(|data_result| -> Vec<PlotSeries> {
                    let mut series = vec![];
                    self.load_series(
                        ctx,
                        query,
                        plot_mem.as_ref(),
                        time_per_pixel,
                        data_result,
                        &mut series,
                    );
                    series
                })
                .collect::<Vec<_>>()
            {
                self.all_series.append(&mut one_series);
            }
        } else {
            let mut series = vec![];
            for data_result in data_results {
                self.load_series(
                    ctx,
                    query,
                    plot_mem.as_ref(),
                    time_per_pixel,
                    data_result,
                    &mut series,
                );
            }
            self.all_series = series;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn load_series(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        plot_mem: Option<&egui_plot::PlotMemory>,
        time_per_pixel: f64,
        data_result: &re_viewer_context::DataResult,
        all_series: &mut Vec<PlotSeries>,
    ) {
        re_tracing::profile_function!();

        let current_query = ctx.current_query();
        let query_ctx = ctx.query_context(data_result, &current_query);

        let fallback_shape = MarkerShape::default();

        let time_offset = ctx
            .view_state
            .downcast_ref::<TimeSeriesViewState>()
            .map_or(0, |state| state.time_offset);
        let time_range =
            determine_time_range(view_query.latest_at, time_offset, data_result, plot_mem);

        {
            use re_view::RangeResultsExt as _;

            re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

            let entity_path = &data_result.entity_path;
            let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range);

            let results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &query,
                data_result,
                [
                    Color::name(),
                    MarkerShape::name(),
                    MarkerSize::name(),
                    Name::name(),
                    Scalar::name(),
                    SeriesVisible::name(),
                ],
            );

            // If we have no scalars, we can't do anything.
            let Some(all_scalar_chunks) = results.get_required_chunks(&Scalar::name()) else {
                return;
            };

            // All the default values for a `PlotPoint`, accounting for both overrides and default values.
            let fallback_color: Color = self.fallback_for(&query_ctx);
            let fallback_size: MarkerSize = self.fallback_for(&query_ctx);
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    color: fallback_color.into(),
                    // NOTE: arguably, the `MarkerSize` value should be twice the `radius_ui`. We do
                    // stick to the semantics of `MarkerSize` == radius for backward compatibility and
                    // because markers need a decent radius value to be at all legible.
                    radius_ui: **fallback_size,
                    kind: PlotSeriesKind::Scatter(ScatterAttrs {
                        marker: fallback_shape,
                    }),
                },
            };

            let num_series = determine_num_series(&all_scalar_chunks);
            let mut points_per_series =
                allocate_plot_points(&query, &default_point, &all_scalar_chunks, num_series);

            collect_scalars(&all_scalar_chunks, &mut points_per_series);
            collect_colors(
                entity_path,
                &query,
                &results,
                &all_scalar_chunks,
                &mut points_per_series,
            );
            collect_radius_ui(
                &query,
                &results,
                &all_scalar_chunks,
                &mut points_per_series,
                MarkerSize::name(),
                // `marker_size` is a radius, see NOTE above
                1.0,
            );

            // Fill in marker shapes
            {
                re_tracing::profile_scope!("fill marker shapes");

                {
                    let all_marker_shapes_chunks =
                        results.get_optional_chunks(&MarkerShape::name());

                    if all_marker_shapes_chunks.len() == 1
                        && all_marker_shapes_chunks[0].is_static()
                    {
                        re_tracing::profile_scope!("override/default fast path");

                        if let Some(marker_shapes) = all_marker_shapes_chunks[0]
                            .iter_component::<MarkerShape>()
                            .next()
                        {
                            for (points, marker_shape) in points_per_series
                                .iter_mut()
                                .zip(clamped_or_nothing(marker_shapes.as_slice(), num_series))
                            {
                                for point in points {
                                    point.attrs.kind = PlotSeriesKind::Scatter(ScatterAttrs {
                                        marker: *marker_shape,
                                    });
                                }
                            }
                        }
                    } else {
                        re_tracing::profile_scope!("standard path");

                        let mut all_marker_shapes_iters = all_marker_shapes_chunks
                            .iter()
                            .map(|chunk| chunk.iter_component::<MarkerShape>())
                            .collect_vec();
                        let all_marker_shapes_indexed = {
                            let all_marker_shapes = all_marker_shapes_iters
                                .iter_mut()
                                .flat_map(|it| it.into_iter());
                            let all_marker_shapes_indices =
                                all_marker_shapes_chunks.iter().flat_map(|chunk| {
                                    chunk.iter_component_indices(
                                        query.timeline(),
                                        &MarkerShape::name(),
                                    )
                                });
                            itertools::izip!(all_marker_shapes_indices, all_marker_shapes)
                        };

                        let all_frames = re_query::range_zip_1x1(
                            all_scalars_indices(&query, &all_scalar_chunks),
                            all_marker_shapes_indexed,
                        )
                        .enumerate();

                        // Simplified path for single series.
                        if num_series == 1 {
                            let points = &mut *points_per_series[0];
                            all_frames.for_each(|(i, (_index, _scalars, marker_shapes))| {
                                if let Some(marker) = marker_shapes
                                    .and_then(|marker_shapes| marker_shapes.first().copied())
                                {
                                    points[i].attrs.kind =
                                        PlotSeriesKind::Scatter(ScatterAttrs { marker });
                                }
                            });
                        } else {
                            all_frames.for_each(|(i, (_index, _scalars, marker_shapes))| {
                                if let Some(marker_shapes) = marker_shapes {
                                    for (points, marker) in points_per_series
                                        .iter_mut()
                                        .zip(clamped_or_nothing(&marker_shapes, num_series))
                                    {
                                        points[i].attrs.kind =
                                            PlotSeriesKind::Scatter(ScatterAttrs {
                                                marker: *marker,
                                            });
                                    }
                                }
                            });
                        }
                    }
                }
            }

            let series_visibility = collect_series_visibility(&query, &results, num_series);
            let series_names = collect_series_name(self, &query_ctx, &results, num_series);

            debug_assert_eq!(points_per_series.len(), series_names.len());
            for (instance, (points, label, visible)) in itertools::izip!(
                points_per_series.into_iter(),
                series_names.into_iter(),
                series_visibility.into_iter()
            )
            .enumerate()
            {
                let instance_path = if num_series == 1 {
                    InstancePath::entity_all(data_result.entity_path.clone())
                } else {
                    InstancePath::instance(
                        data_result.entity_path.clone(),
                        (instance as u64).into(),
                    )
                };

                points_to_series(
                    instance_path,
                    time_per_pixel,
                    visible,
                    points,
                    ctx.recording_engine().store(),
                    view_query,
                    label,
                    // Aggregation for points is not supported.
                    re_types::components::AggregationPolicy::Off,
                    all_series,
                );
            }
        }
    }
}
