use itertools::Itertools as _;

use re_query::{PromiseResult, QueryError};
use re_space_view::range_with_overrides;
use re_types::{
    archetypes::{self, SeriesPoint},
    components::{Color, MarkerShape, MarkerSize, Name, Scalar},
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
use crate::ScatterAttrs;
use crate::{PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

/// The system for rendering [`SeriesPoint`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesPointSystem {
    pub all_series: Vec<PlotSeries>,
}

impl IdentifiedViewSystem for SeriesPointSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SeriesPoint".into()
    }
}

// We use a larger default stroke width for scatter plots so the marker is
// visible.
const DEFAULT_MARKER_SIZE: f32 = 3.0;

impl VisualizerSystem for SeriesPointSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        let mut query_info = VisualizerQueryInfo::from_archetype::<archetypes::Scalar>();
        let mut series_point_queried: ComponentNameSet = SeriesPoint::all_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect::<ComponentNameSet>();
        query_info.queried.append(&mut series_point_queried);
        query_info.indicators = std::iter::once(SeriesPoint::indicator().name()).collect();
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

impl TypedComponentFallbackProvider<Color> for SeriesPointSystem {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        fallback_color(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<MarkerSize> for SeriesPointSystem {
    fn fallback_for(&self, _ctx: &QueryContext<'_>) -> MarkerSize {
        MarkerSize(DEFAULT_MARKER_SIZE)
    }
}

re_viewer_context::impl_component_fallback_provider!(SeriesPointSystem => [Color, MarkerSize]);

impl SeriesPointSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let resolver = ctx.recording().resolver();

        let (plot_bounds, time_per_pixel) =
            determine_plot_bounds_and_time_per_pixel(ctx.viewer_ctx, view_query);

        // TODO(cmc): this should be thread-pooled in case there are a gazillon series in the same plot…
        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let query_ctx = QueryContext {
                view_ctx: ctx,
                archetype_name: Some(SeriesPoint::name()),
                query: &ctx.current_query(),
                target_entity_path: &data_result.entity_path,
            };

            let fallback_color =
                re_viewer_context::TypedComponentFallbackProvider::<Color>::fallback_for(
                    self, &query_ctx,
                );

            let fallback_size =
                re_viewer_context::TypedComponentFallbackProvider::<MarkerSize>::fallback_for(
                    self, &query_ctx,
                );

            let fallback_shape = MarkerShape::default();

            // All the default values for a `PlotPoint`, accounting for both overrides and default
            // values.
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    label: None,
                    color: fallback_color.into(),
                    marker_size: fallback_size.into(),
                    kind: PlotSeriesKind::Scatter(ScatterAttrs {
                        marker: fallback_shape,
                    }),
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
                        MarkerSize::name(),
                        MarkerShape::name(),
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

                // Make it as clear as possible to the optimizer that some parameters
                // go completely unused as soon as overrides have been defined.

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

                // Fill in marker sizes
                // TODO(jleibs): Handle Err values.
                if let Ok(all_marker_sizes) = results.get_or_empty_dense::<MarkerSize>(resolver) {
                    if !matches!(
                        all_marker_sizes.status(),
                        (PromiseResult::Ready(()), PromiseResult::Ready(()))
                    ) {
                        // TODO(#5607): what should happen if the promise is still pending?
                    }

                    let all_scalars_indexed = all_scalars
                        .range_indices(all_scalars_entry_range.clone())
                        .map(|index| (index, ()));

                    let all_frames = re_query::range_zip_1x1(
                        all_scalars_indexed,
                        all_marker_sizes.range_indexed(),
                    )
                    .enumerate();

                    for (i, (_index, _scalars, marker_sizes)) in all_frames {
                        if let Some(marker_size) =
                            marker_sizes.and_then(|marker_sizes| marker_sizes.first().copied())
                        {
                            points[i].attrs.marker_size = marker_size.0;
                        }
                    }
                }

                // Fill in marker sizes
                // TODO(jleibs): Handle Err values.
                if let Ok(all_marker_shapes) = results.get_or_empty_dense::<MarkerShape>(resolver) {
                    if !matches!(
                        all_marker_shapes.status(),
                        (PromiseResult::Ready(()), PromiseResult::Ready(()))
                    ) {
                        // TODO(#5607): what should happen if the promise is still pending?
                    }

                    let all_scalars_indexed = all_scalars
                        .range_indices(all_scalars_entry_range.clone())
                        .map(|index| (index, ()));

                    let all_frames = re_query::range_zip_1x1(
                        all_scalars_indexed,
                        all_marker_shapes.range_indexed(),
                    )
                    .enumerate();

                    for (i, (_index, _scalars, marker_shapes)) in all_frames {
                        if let Some(marker) =
                            marker_shapes.and_then(|marker_shapes| marker_shapes.first().copied())
                        {
                            points[i].attrs.kind = PlotSeriesKind::Scatter(ScatterAttrs { marker });
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
                    &mut self.all_series,
                );
            }
        }

        Ok(())
    }
}
