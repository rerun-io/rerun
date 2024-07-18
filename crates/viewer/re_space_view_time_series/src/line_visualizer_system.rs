use itertools::{Either, Itertools as _};
use re_query2::{PromiseResult, QueryError};
use re_space_view::{range_with_blueprint_resolved_data2, RangeResultsExt2};
use re_types::archetypes;
use re_types::components::AggregationPolicy;
use re_types::external::arrow2::array::PrimitiveArray;
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

        match self.load_scalars(ctx, query) {
            Ok(_) | Err(QueryError::PrimaryNotFound(_)) => Ok(Vec::new()),
            Err(err) => Err(err.into()),
        }
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

impl TypedComponentFallbackProvider<Name> for SeriesLineSystem {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Name {
        ctx.target_entity_path
            .last()
            .map(|part| part.ui_string().into())
            .unwrap_or_default()
    }
}

re_viewer_context::impl_component_fallback_provider!(SeriesLineSystem => [Color, StrokeWidth, Name]);

impl SeriesLineSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let (plot_bounds, time_per_pixel) =
            determine_plot_bounds_and_time_per_pixel(ctx.viewer_ctx, query);

        let data_results = query.iter_visible_data_results(ctx, Self::identifier());

        let parallel_loading = false; // TODO(emilk): enable parallel loading when it is faster, because right now it is often slower.
        if parallel_loading {
            use rayon::prelude::*;
            re_tracing::profile_wait!("load_series");
            for one_series in data_results
                .collect_vec()
                .par_iter()
                .map(|data_result| -> Result<Vec<PlotSeries>, QueryError> {
                    let mut series = vec![];
                    self.load_series(
                        ctx,
                        query,
                        plot_bounds,
                        time_per_pixel,
                        data_result,
                        &mut series,
                    )?;
                    Ok(series)
                })
                .collect::<Vec<Result<_, _>>>()
            {
                self.all_series.append(&mut one_series?);
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
                )?;
            }
            self.all_series = series;
        }

        Ok(())
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
    ) -> Result<(), QueryError> {
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
            use re_space_view::RangeResultsExt2 as _;

            re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

            let entity_path = &data_result.entity_path;
            let query = re_chunk_store::RangeQuery::new(view_query.timeline, time_range);

            let results = range_with_blueprint_resolved_data2(
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
                return Ok(());
            };

            // Allocate all points.
            points = all_scalar_chunks
                .iter()
                .flat_map(|chunk| chunk.iter_component_indices(&query.timeline(), &Scalar::name()))
                .map(|(data_time, _)| PlotPoint {
                    time: data_time.as_i64(),
                    ..default_point.clone()
                })
                .collect_vec();

            // TODO: asserting Scalar == f64 would be nice.

            // Fill in values.
            let mut i = 0;
            for chunk in all_scalar_chunks.iter() {
                for ((_idx, len), values) in chunk.iter_primitive::<f64>(&Scalar::name()) {
                    if len > 0 {
                        if len > 1 {
                            re_log::warn_once!(
                                "found a scalar batch in {entity_path:?} -- those have no effect"
                            );
                        }

                        points[i].value = values[0];
                    } else {
                        points[i].attrs.kind = PlotSeriesKind::Clear;
                    }

                    i += 1;
                }
            }

            // Make it as clear as possible to the optimizer that some parameters
            // go completely unused as soon as overrides have been defined.

            // Fill in colors.
            // TODO(jleibs): Handle Err values.
            // TODO: asserting Color == u32 would be nice.
            if let Some(all_color_chunks) = results.get_required_chunks(&Color::name()) {
                let all_scalars_indexed = all_scalar_chunks
                    .iter()
                    .flat_map(|chunk| {
                        // TODO: this should proabably be a helper method, really
                        chunk.iter_component_indices(&query.timeline(), &Scalar::name())
                    })
                    .map(|index| (index, ()));

                let all_colors = all_color_chunks.iter().flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(&query.timeline(), &Color::name()),
                        chunk
                            .iter_primitive::<u32>(&Color::name())
                            .map(|(_offsets, values)| values)
                    )
                });
                // .map(|(index, (offset, values)));

                let all_frames =
                    re_query2::range_zip_1x1(all_scalars_indexed, all_colors).enumerate();

                for (i, (_index, _scalars, colors)) in all_frames {
                    if let Some(color) = colors.and_then(|colors| {
                        colors.first().map(|c| {
                            let [a, b, g, r] = c.to_le_bytes();
                            if a == 255 {
                                // Common-case optimization
                                re_renderer::Color32::from_rgb(r, g, b)
                            } else {
                                re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
                            }
                        })
                    }) {
                        points[i].attrs.color = color;
                    }
                }
            }

            // Fill in stroke widths
            // TODO(jleibs): Handle Err values.
            // TODO: asserting StrokeWidth == f32 would be nice.
            if let Some(all_stroke_width_chunks) = results.get_required_chunks(&StrokeWidth::name())
            {
                let all_scalars_indexed = all_scalar_chunks
                    .iter()
                    .flat_map(|chunk| {
                        chunk.iter_component_indices(&query.timeline(), &Scalar::name())
                    })
                    .map(|index| (index, ()));

                let all_stroke_widths = all_stroke_width_chunks.iter().flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(&query.timeline(), &StrokeWidth::name()),
                        chunk
                            .iter_primitive::<f32>(&StrokeWidth::name())
                            .map(|(_offsets, values)| values)
                    )
                });

                let all_frames =
                    re_query2::range_zip_1x1(all_scalars_indexed, all_stroke_widths).enumerate();

                for (i, (_index, _scalars, stroke_widths)) in all_frames {
                    if let Some(stroke_width) =
                        stroke_widths.and_then(|stroke_widths| stroke_widths.first().copied())
                    {
                        points[i].attrs.radius_ui = 0.5 * stroke_width;
                    }
                }
            }

            // Extract the series name
            let series_name = results
                .get_optional_chunks(&Name::name())
                .iter()
                .find(|chunk| !chunk.is_empty())
                .and_then(|chunk| chunk.component_mono::<Name>(0))
                .unwrap_or_else(|| self.fallback_for(&query_ctx));

            // Now convert the `PlotPoints` into `Vec<PlotSeries>`
            let aggregator = results
                .get_optional_chunks(&AggregationPolicy::name())
                .iter()
                .find(|chunk| !chunk.is_empty())
                .and_then(|chunk| chunk.component_mono::<AggregationPolicy>(0))
                // TODO(andreas): Relying on the default==placeholder here instead of going through a fallback provider.
                //                This is fine, because we know there's no `TypedFallbackProvider`, but wrong if one were to be added.
                .unwrap_or_default();

            // TODO:sort?

            // This is _almost_ sorted already: all the individual chunks are sorted, but we still
            // have to deal with overlap chunks.
            {
                re_tracing::profile_scope!("sort");
                points.sort_by_key(|p| p.time);
            }

            points_to_series(
                &data_result.entity_path,
                time_per_pixel,
                points,
                ctx.recording_store(),
                view_query,
                &series_name,
                aggregator,
                all_series,
            );
        }

        Ok(())
    }
}
