use itertools::Itertools as _;

use re_chunk_store::{RangeQuery, RowId};
use re_log_types::{EntityPath, TimeInt};
use re_types::archetypes;
use re_types::components::{AggregationPolicy, ClearIsRecursive, SeriesVisible};
use re_types::{
    components::{Color, Name, Scalar, StrokeWidth},
    Archetype as _, Component as _,
};
use re_view::range_with_blueprint_resolved_data;
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, TypedComponentFallbackProvider,
    ViewContext, ViewQuery, ViewStateExt as _, ViewSystemExecutionError, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::series_query::{
    allocate_plot_points, collect_colors, collect_radius_ui, collect_scalars, collect_series_name,
    collect_series_visibility, determine_num_series,
};
use crate::util::{determine_time_per_pixel, determine_time_range, points_to_series};
use crate::view_class::TimeSeriesViewState;
use crate::{PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

/// The system for rendering [`archetypes::SeriesLines`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesLineSystem {
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesLineSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesLines".into()
    }
}

const DEFAULT_STROKE_WIDTH: f32 = 0.75;

impl VisualizerSystem for SeriesLineSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<archetypes::Scalars>();
        query_info.queried.extend(
            archetypes::SeriesLines::all_components()
                .iter()
                .map(|descr| descr.component_name),
        );

        query_info.indicators =
            [archetypes::SeriesLines::descriptor_indicator().component_name].into();

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

impl TypedComponentFallbackProvider<Color> for SeriesLineSystem {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<StrokeWidth> for SeriesLineSystem {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> StrokeWidth {
        StrokeWidth(DEFAULT_STROKE_WIDTH.into())
    }
}

impl TypedComponentFallbackProvider<Name> for SeriesLineSystem {
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

impl TypedComponentFallbackProvider<SeriesVisible> for SeriesLineSystem {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> SeriesVisible {
        true.into()
    }
}

re_viewer_context::impl_component_fallback_provider!(SeriesLineSystem => [Color, StrokeWidth, Name, SeriesVisible]);

impl SeriesLineSystem {
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
            let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range)
                // We must fetch data with extended bounds, otherwise the query clamping would
                // cut-off the data early at the edge of the view.
                .include_extended_bounds(true);

            let results = range_with_blueprint_resolved_data(
                ctx,
                None,
                &query,
                data_result,
                [
                    Scalar::name(),
                    Color::name(),
                    StrokeWidth::name(),
                    Name::name(),
                    AggregationPolicy::name(),
                    SeriesVisible::name(),
                ],
            );

            // If we have no scalars, we can't do anything.
            let Some(all_scalar_chunks) = results.get_required_chunks(&Scalar::name()) else {
                return;
            };

            // All the default values for a `PlotPoint`, accounting for both overrides and default values.
            // TODO(andreas): Fallback should produce several colors. Instead, we generate additional ones on the fly if necessary right now.
            let fallback_color: Color = self.fallback_for(&query_ctx);
            let fallback_stroke_width: StrokeWidth = self.fallback_for(&query_ctx);
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    color: fallback_color.into(),
                    radius_ui: 0.5 * *fallback_stroke_width.0,
                    kind: PlotSeriesKind::Continuous,
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
                StrokeWidth::name(),
                0.5,
            );

            // Now convert the `PlotPoints` into `Vec<PlotSeries>`
            let aggregator = results
                .get_optional_chunks(&AggregationPolicy::name())
                .iter()
                .find(|chunk| !chunk.is_empty())
                .and_then(|chunk| chunk.component_mono::<AggregationPolicy>(0)?.ok())
                // TODO(andreas): Relying on the default==placeholder here instead of going through a fallback provider.
                //                This is fine, because we know there's no `TypedFallbackProvider`, but wrong if one were to be added.
                .unwrap_or_default();

            // NOTE: The chunks themselves are already sorted as best as possible (hint: overlap)
            // by the query engine.
            let all_chunks_sorted_and_not_overlapped =
                all_scalar_chunks.iter().tuple_windows().all(|(lhs, rhs)| {
                    let lhs_time_max = lhs
                        .timelines()
                        .get(query.timeline())
                        .map_or(TimeInt::MAX, |time_column| time_column.time_range().max());
                    let rhs_time_min = rhs
                        .timelines()
                        .get(query.timeline())
                        .map_or(TimeInt::MIN, |time_column| time_column.time_range().min());
                    lhs_time_max <= rhs_time_min
                });

            let has_discontinuities = {
                // Find all clears that may apply, in order to render discontinuities properly.

                re_tracing::profile_scope!("discontinuities");

                let cleared_indices = collect_recursive_clears(ctx, &query, entity_path);
                let has_discontinuities = !cleared_indices.is_empty();

                for points in &mut points_per_series {
                    points.extend(cleared_indices.iter().map(|(data_time, _)| PlotPoint {
                        time: data_time.as_i64(),
                        value: 0.0,
                        attrs: PlotPointAttrs {
                            color: egui::Color32::TRANSPARENT,
                            radius_ui: 0.0,
                            kind: PlotSeriesKind::Clear,
                        },
                    }));
                }

                has_discontinuities
            };

            // This is _almost_ sorted already: all the individual chunks are sorted, but we still
            // have to deal with overlapped chunks, or discontinuities introduced by query-time clears.
            if !all_chunks_sorted_and_not_overlapped || has_discontinuities {
                re_tracing::profile_scope!("sort");
                for points in &mut points_per_series {
                    points.sort_by_key(|p| p.time);
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
                    aggregator,
                    all_series,
                );
            }
        }
    }
}

fn collect_recursive_clears(
    ctx: &ViewContext<'_>,
    query: &RangeQuery,
    entity_path: &EntityPath,
) -> Vec<(TimeInt, RowId)> {
    re_tracing::profile_function!();

    let mut cleared_indices = Vec::new();

    let mut clear_entity_path = entity_path.clone();
    loop {
        let results = ctx.recording_engine().cache().range(
            query,
            &clear_entity_path,
            [ClearIsRecursive::name()],
        );

        let empty = Vec::new();
        let chunks = results
            .components
            .get(&ClearIsRecursive::name())
            .unwrap_or(&empty);

        for chunk in chunks {
            cleared_indices.extend(
                itertools::izip!(
                    chunk.iter_component_indices(query.timeline(), &ClearIsRecursive::name()),
                    chunk
                        .iter_component::<ClearIsRecursive>()
                        .map(|is_recursive| {
                            is_recursive.as_slice().first().is_some_and(|v| *v.0)
                        })
                )
                .filter_map(|(index, is_recursive)| {
                    let is_recursive = is_recursive || clear_entity_path == *entity_path;
                    is_recursive.then_some(index)
                }),
            );
        }

        let Some(parent_entity_path) = clear_entity_path.parent() else {
            break;
        };

        clear_entity_path = parent_entity_path;
    }

    cleared_indices
}
