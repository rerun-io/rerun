use itertools::Itertools;

use re_log_types::TimeInt;
use re_space_view::range_with_blueprint_resolved_data;
use re_types::archetypes;
use re_types::components::AggregationPolicy;
use re_types::external::arrow2::datatypes::DataType as ArrowDatatype;
use re_types::{
    archetypes::SeriesLine,
    components::{Color, Name, Scalar, StrokeWidth},
    Archetype as _, Loggable,
};
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    TypedComponentFallbackProvider, ViewContext, ViewQuery, VisualizerQueryInfo, VisualizerSystem,
};

use crate::util::{
    determine_plot_bounds_and_time_per_pixel, determine_time_range, points_to_series,
};
use crate::{PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

/// The system for rendering [`SeriesLine`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesLineSystem {
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesLineSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesLine".into()
    }
}

const DEFAULT_STROKE_WIDTH: f32 = 0.75;

impl VisualizerSystem for SeriesLineSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<archetypes::Scalar>();
        query_info
            .queried
            .extend(SeriesLine::all_components().iter().map(ToOwned::to_owned));
        query_info.indicators = std::iter::once(SeriesLine::indicator().name()).collect();
        query_info
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.load_scalars(ctx, query);
        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
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

re_viewer_context::impl_component_fallback_provider!(SeriesLineSystem => [Color, StrokeWidth]);

impl SeriesLineSystem {
    fn load_scalars(&mut self, ctx: &ViewContext<'_>, query: &ViewQuery<'_>) {
        re_tracing::profile_function!();

        let (plot_bounds, time_per_pixel) =
            determine_plot_bounds_and_time_per_pixel(ctx.viewer_ctx, query);

        let data_results = query.iter_visible_data_results(ctx, Self::identifier());

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
                        plot_bounds,
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
                    plot_bounds,
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
        plot_bounds: Option<egui_plot::PlotBounds>,
        time_per_pixel: f64,
        data_result: &re_viewer_context::DataResult,
        all_series: &mut Vec<PlotSeries>,
    ) {
        re_tracing::profile_function!();

        let current_query = ctx.current_query();
        let query_ctx = ctx.query_context(data_result, &current_query);

        let fallback_color: Color = self.fallback_for(&query_ctx);
        let fallback_stroke_width: StrokeWidth = self.fallback_for(&query_ctx);

        // All the default values for a `PlotPoint`, accounting for both overrides and default
        // values.
        let default_point = PlotPoint {
            time: 0,
            value: 0.0,
            attrs: PlotPointAttrs {
                color: fallback_color.into(),
                radius_ui: 0.5 * *fallback_stroke_width.0,
                kind: PlotSeriesKind::Continuous,
            },
        };

        let mut points;

        let time_range = determine_time_range(
            view_query.latest_at,
            data_result,
            plot_bounds,
            ctx.viewer_ctx.app_options.experimental_plot_query_clamping,
        );
        {
            use re_space_view::RangeResultsExt as _;

            re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

            let entity_path = &data_result.entity_path;
            let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range);

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
                ],
            );

            // If we have no scalars, we can't do anything.
            let Some(all_scalar_chunks) = results.get_required_chunks(&Scalar::name()) else {
                return;
            };

            let all_scalars_indices = || {
                all_scalar_chunks
                    .iter()
                    .flat_map(|chunk| {
                        chunk.iter_component_indices(&query.timeline(), &Scalar::name())
                    })
                    // That is just so we can satisfy the `range_zip` contract later on.
                    .map(|index| (index, ()))
            };

            // Allocate all points.
            {
                re_tracing::profile_scope!("alloc");

                points = all_scalar_chunks
                    .iter()
                    .flat_map(|chunk| {
                        chunk.iter_component_indices(&query.timeline(), &Scalar::name())
                    })
                    .map(|(data_time, _)| {
                        debug_assert_eq!(Scalar::arrow_datatype(), ArrowDatatype::Float64);

                        PlotPoint {
                            time: data_time.as_i64(),
                            ..default_point.clone()
                        }
                    })
                    .collect_vec();
            }

            // Fill in values.
            {
                re_tracing::profile_scope!("fill values");

                debug_assert_eq!(Scalar::arrow_datatype(), ArrowDatatype::Float64);
                all_scalar_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_primitive::<f64>(&Scalar::name()))
                    .enumerate()
                    .for_each(|(i, values)| {
                        if !values.is_empty() {
                            if values.len() > 1 {
                                re_log::warn_once!(
                                    "found a scalar batch in {entity_path:?} -- those have no effect"
                                );
                            }

                            points[i].value = values[0];
                        } else {
                            points[i].attrs.kind = PlotSeriesKind::Clear;
                        }
                    });
            }

            // Fill in colors.
            {
                re_tracing::profile_scope!("fill colors");

                debug_assert_eq!(Color::arrow_datatype(), ArrowDatatype::UInt32);

                fn map_raw_color(raw: &[u32]) -> Option<re_renderer::Color32> {
                    raw.first().map(|c| {
                        let [a, b, g, r] = c.to_le_bytes();
                        if a == 255 {
                            // Common-case optimization
                            re_renderer::Color32::from_rgb(r, g, b)
                        } else {
                            re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
                        }
                    })
                }

                if let Some(all_color_chunks) = results.get_required_chunks(&Color::name()) {
                    if all_color_chunks.len() == 1 && all_color_chunks[0].is_static() {
                        re_tracing::profile_scope!("override fast path");

                        let color = all_color_chunks[0]
                            .iter_primitive::<u32>(&Color::name())
                            .next()
                            .and_then(map_raw_color);

                        if let Some(color) = color {
                            points.iter_mut().for_each(|p| p.attrs.color = color);
                        }
                    } else {
                        re_tracing::profile_scope!("standard path");

                        let all_colors = all_color_chunks.iter().flat_map(|chunk| {
                            itertools::izip!(
                                chunk.iter_component_indices(&query.timeline(), &Color::name()),
                                chunk.iter_primitive::<u32>(&Color::name())
                            )
                        });

                        let all_frames =
                            re_query::range_zip_1x1(all_scalars_indices(), all_colors).enumerate();

                        all_frames.for_each(|(i, (_index, _scalars, colors))| {
                            if let Some(color) = colors.and_then(map_raw_color) {
                                points[i].attrs.color = color;
                            }
                        });
                    }
                }
            }

            // Fill in stroke widths
            {
                re_tracing::profile_scope!("fill stroke widths");

                debug_assert_eq!(StrokeWidth::arrow_datatype(), ArrowDatatype::Float32);

                if let Some(all_stroke_width_chunks) =
                    results.get_required_chunks(&StrokeWidth::name())
                {
                    if all_stroke_width_chunks.len() == 1 && all_stroke_width_chunks[0].is_static()
                    {
                        re_tracing::profile_scope!("override fast path");

                        let stroke_width = all_stroke_width_chunks[0]
                            .iter_primitive::<f32>(&StrokeWidth::name())
                            .next()
                            .and_then(|stroke_widths| stroke_widths.first().copied());

                        if let Some(stroke_width) = stroke_width {
                            points
                                .iter_mut()
                                .for_each(|p| p.attrs.radius_ui = stroke_width * 0.5);
                        }
                    } else {
                        re_tracing::profile_scope!("standard path");

                        let all_stroke_widths = all_stroke_width_chunks.iter().flat_map(|chunk| {
                            itertools::izip!(
                                chunk.iter_component_indices(
                                    &query.timeline(),
                                    &StrokeWidth::name()
                                ),
                                chunk.iter_primitive::<f32>(&StrokeWidth::name())
                            )
                        });

                        let all_frames =
                            re_query::range_zip_1x1(all_scalars_indices(), all_stroke_widths)
                                .enumerate();

                        all_frames.for_each(|(i, (_index, _scalars, stroke_widths))| {
                            if let Some(stroke_width) = stroke_widths
                                .and_then(|stroke_widths| stroke_widths.first().copied())
                            {
                                points[i].attrs.radius_ui = stroke_width * 0.5;
                            }
                        });
                    }
                }
            }

            // Extract the series name
            let series_name = results
                .get_optional_chunks(&Name::name())
                .iter()
                .find(|chunk| !chunk.is_empty())
                .and_then(|chunk| chunk.component_mono::<Name>(0)?.ok())
                .map(|name| name.0.to_string());

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
                        .get(&query.timeline())
                        .map_or(TimeInt::MAX, |time_column| time_column.time_range().max());
                    let rhs_time_min = rhs
                        .timelines()
                        .get(&query.timeline())
                        .map_or(TimeInt::MIN, |time_column| time_column.time_range().min());
                    lhs_time_max <= rhs_time_min
                });

            // This is _almost_ sorted already: all the individual chunks are sorted, but we still
            // have to deal with overlap chunks.
            if !all_chunks_sorted_and_not_overlapped {
                re_tracing::profile_scope!("sort");
                points.sort_by_key(|p| p.time);
            }

            points_to_series(
                &data_result.entity_path,
                time_per_pixel,
                points,
                ctx.recording_store(),
                view_query,
                series_name,
                aggregator,
                all_series,
            );
        }
    }
}
