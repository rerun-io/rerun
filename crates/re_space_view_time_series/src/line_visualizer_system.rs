use itertools::Itertools as _;
use re_query::{PromiseResult, QueryError};
use re_space_view::range_with_overrides;
use re_types::archetypes;
use re_types::{
    archetypes::SeriesLine,
    components::{Color, Name, Scalar, StrokeWidth},
    Archetype as _, ComponentNameSet, Loggable,
};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    TypedComponentFallbackProvider, ViewContext, ViewQuery, VisualizerQueryInfo, VisualizerSystem,
};

use crate::overrides::fallback_color;
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
        let mut series_line_queried: ComponentNameSet = SeriesLine::all_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect::<ComponentNameSet>();
        query_info.queried.append(&mut series_line_queried);
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
        fallback_color(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<StrokeWidth> for SeriesLineSystem {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> StrokeWidth {
        StrokeWidth(DEFAULT_STROKE_WIDTH)
    }
}

re_viewer_context::impl_component_fallback_provider!(SeriesLineSystem => [Color, StrokeWidth]);

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

        let resolver = ctx.recording().resolver();

        let query_ctx = QueryContext {
            view_ctx: ctx,
            archetype_name: Some(SeriesLine::name()),
            query: &ctx.current_query(),
            target_entity_path: &data_result.entity_path,
        };

        let fallback_color =
            re_viewer_context::TypedComponentFallbackProvider::<Color>::fallback_for(
                self, &query_ctx,
            );

        let fallback_stroke =
            re_viewer_context::TypedComponentFallbackProvider::<StrokeWidth>::fallback_for(
                self, &query_ctx,
            );

        // All the default values for a `PlotPoint`, accounting for both overrides and default
        // values.
        let default_point = PlotPoint {
            time: 0,
            value: 0.0,
            attrs: PlotPointAttrs {
                label: None,
                color: fallback_color.into(),
                marker_size: fallback_stroke.into(),
                kind: PlotSeriesKind::Continuous,
            },
        };

        let mut points;
        let mut series_name = Default::default();

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
            let query = re_data_store::RangeQuery::new(view_query.timeline, time_range);

            let results = range_with_overrides(
                ctx.viewer_ctx,
                None,
                &query,
                data_result,
                [
                    Scalar::name(),
                    Color::name(),
                    StrokeWidth::name(),
                    Name::name(),
                ],
            );

            // If we have no scalars, we can't do anything.
            let Some(all_scalars) = results.get_dense::<Scalar>(resolver) else {
                return Ok(());
            };

            let all_scalars = all_scalars?;

            let all_scalars_entry_range = all_scalars.entry_range();

            if !matches!(
                all_scalars.status(),
                (PromiseResult::Ready(()), PromiseResult::Ready(()))
            ) {
                // TODO(#5607): what should happen if the promise is still pending?
            }

            // Allocate all points.
            points = all_scalars
                .range_indices(all_scalars_entry_range.clone())
                .map(|(data_time, _)| PlotPoint {
                    time: data_time.as_i64(),
                    ..default_point.clone()
                })
                .collect_vec();

            if cfg!(debug_assertions) {
                for ps in points.windows(2) {
                    assert!(
                    ps[0].time <= ps[1].time,
                    "scalars should be sorted already when extracted from the cache, got p0 at {} and p1 at {}\n{:?}",
                    ps[0].time, ps[1].time,
                    points.iter().map(|p| p.time).collect_vec(),
                );
                }
            }

            // Fill in values.
            for (i, scalars) in all_scalars
                .range_data(all_scalars_entry_range.clone())
                .enumerate()
            {
                if scalars.len() > 1 {
                    re_log::warn_once!(
                        "found a scalar batch in {entity_path:?} -- those have no effect"
                    );
                } else if scalars.is_empty() {
                    points[i].attrs.kind = PlotSeriesKind::Clear;
                } else {
                    points[i].value = scalars.first().map_or(0.0, |s| s.0);
                }
            }

            // Fill in colors.
            // TODO(jleibs): Handle Err values.
            if let Ok(all_colors) = results.get_or_empty_dense::<Color>(resolver) {
                if !matches!(
                    all_colors.status(),
                    (PromiseResult::Ready(()), PromiseResult::Ready(()))
                ) {
                    // TODO(#5607): what should happen if the promise is still pending?
                }

                let all_scalars_indexed = all_scalars
                    .range_indices(all_scalars_entry_range.clone())
                    .map(|index| (index, ()));

                let all_frames =
                    re_query::range_zip_1x1(all_scalars_indexed, all_colors.range_indexed())
                        .enumerate();

                for (i, (_index, _scalars, colors)) in all_frames {
                    if let Some(color) = colors.and_then(|colors| {
                        colors.first().map(|c| {
                            let [r, g, b, a] = c.to_array();
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
            if let Ok(all_stroke_widths) = results.get_or_empty_dense::<StrokeWidth>(resolver) {
                if !matches!(
                    all_stroke_widths.status(),
                    (PromiseResult::Ready(()), PromiseResult::Ready(()))
                ) {
                    // TODO(#5607): what should happen if the promise is still pending?
                }

                let all_scalars_indexed = all_scalars
                    .range_indices(all_scalars_entry_range.clone())
                    .map(|index| (index, ()));

                let all_frames =
                    re_query::range_zip_1x1(all_scalars_indexed, all_stroke_widths.range_indexed())
                        .enumerate();

                for (i, (_index, _scalars, stroke_widths)) in all_frames {
                    if let Some(stroke_width) =
                        stroke_widths.and_then(|stroke_widths| stroke_widths.first().map(|r| r.0))
                    {
                        points[i].attrs.marker_size = stroke_width;
                    }
                }
            }

            // Extract the series name
            // TODO(jleibs): Handle Err values.
            if let Ok(all_series_name) = results.get_or_empty_dense::<Name>(resolver) {
                if !matches!(
                    all_series_name.status(),
                    (PromiseResult::Ready(()), PromiseResult::Ready(()))
                ) {
                    // TODO(#5607): what should happen if the promise is still pending?
                }

                series_name = all_series_name
                    .range_data(all_scalars_entry_range.clone())
                    .next()
                    .and_then(|name| name.first())
                    .map(|name| name.0.clone());
            }

            // Now convert the `PlotPoints` into `Vec<PlotSeries>`
            points_to_series(
                data_result,
                time_per_pixel,
                points,
                ctx.recording_store(),
                view_query,
                series_name,
                all_series,
            );
        }

        Ok(())
    }
}
