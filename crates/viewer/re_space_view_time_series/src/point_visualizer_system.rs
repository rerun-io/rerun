use itertools::{Either, Itertools as _};

use re_query2::{PromiseResult, QueryError};
use re_space_view::range_with_blueprint_resolved_data2;
use re_types::{
    archetypes::{self, SeriesPoint},
    components::{Color, MarkerShape, MarkerSize, Name, Scalar},
    external::arrow2::array::PrimitiveArray,
    Archetype as _, Loggable as _,
};
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    TypedComponentFallbackProvider, ViewContext, ViewQuery, VisualizerQueryInfo, VisualizerSystem,
};

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
        query_info
            .queried
            .extend(SeriesPoint::all_components().iter().map(ToOwned::to_owned));
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
        ctx.target_entity_path
            .last()
            .map(|part| part.ui_string().into())
            .unwrap_or_default()
    }
}

re_viewer_context::impl_component_fallback_provider!(SeriesPointSystem => [Color, MarkerSize, Name]);

impl SeriesPointSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let (plot_bounds, time_per_pixel) =
            determine_plot_bounds_and_time_per_pixel(ctx.viewer_ctx, view_query);

        // TODO(cmc): this should be thread-pooled in case there are a gazillon series in the same plot…
        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let current_query = ctx.current_query();
            let query_ctx = ctx.query_context(data_result, &current_query);

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
                    color: fallback_color.into(),
                    radius_ui: **fallback_size,
                    kind: PlotSeriesKind::Scatter(ScatterAttrs {
                        marker: fallback_shape,
                    }),
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
                        Color::name(),
                        MarkerShape::name(),
                        MarkerSize::name(),
                        Name::name(),
                        Scalar::name(),
                    ],
                );

                // If we have no scalars, we can't do anything.
                let Some(all_scalar_chunks) = results.get_required_chunks(&Scalar::name()) else {
                    return Ok(());
                };

                // Allocate all points.
                points = all_scalar_chunks
                    .iter()
                    .flat_map(|chunk| {
                        // TODO: this should proabably be a helper method, really
                        chunk.iter_component_indices(&query.timeline(), &Scalar::name())
                    })
                    .map(|(data_time, _)| PlotPoint {
                        time: data_time.as_i64(),
                        ..default_point.clone()
                    })
                    .collect_vec();

                // TODO: probably doesnt make sense anymore
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

                // Fill in marker sizes
                // TODO(jleibs): Handle Err values.
                // TODO: asserting MarkerSize == f32 would be nice.
                if let Some(all_marker_size_chunks) =
                    results.get_required_chunks(&MarkerSize::name())
                {
                    let all_scalars_indexed = all_scalar_chunks
                        .iter()
                        .flat_map(|chunk| {
                            chunk.iter_component_indices(&query.timeline(), &Scalar::name())
                        })
                        .map(|index| (index, ()));

                    let all_marker_sizes = all_marker_size_chunks.iter().flat_map(|chunk| {
                        itertools::izip!(
                            chunk.iter_component_indices(&query.timeline(), &MarkerSize::name()),
                            chunk
                                .iter_primitive::<f32>(&MarkerSize::name())
                                .map(|(_offsets, values)| values)
                        )
                    });

                    let all_frames =
                        re_query2::range_zip_1x1(all_scalars_indexed, all_marker_sizes).enumerate();

                    for (i, (_index, _scalars, marker_sizes)) in all_frames {
                        if let Some(marker_size) =
                            marker_sizes.and_then(|marker_sizes| marker_sizes.first().copied())
                        {
                            points[i].attrs.radius_ui = marker_size;
                        }
                    }
                }

                // Fill in marker shapes
                // TODO(jleibs): Handle Err values.
                if let Some(all_marker_shape_chunks) =
                    results.get_required_chunks(&MarkerShape::name())
                {
                    let all_scalars_indexed = all_scalar_chunks
                        .iter()
                        .flat_map(|chunk| {
                            chunk.iter_component_indices(&query.timeline(), &Scalar::name())
                        })
                        .map(|index| (index, ()));

                    let mut all_marker_shape_iters = all_marker_shape_chunks
                        .iter()
                        .map(|chunk| chunk.iter_component::<MarkerShape>())
                        .collect_vec();
                    let all_marker_shape_iters = all_marker_shape_iters
                        .iter_mut()
                        .map(|iter| iter.into_iter())
                        .collect_vec();

                    let all_marker_shapes =
                        itertools::izip!(all_marker_shape_chunks.iter(), all_marker_shape_iters)
                            .flat_map(|(chunk, iter)| {
                                itertools::izip!(
                                    chunk.iter_component_indices(
                                        &query.timeline(),
                                        &MarkerShape::name()
                                    ),
                                    iter.map(|(_offsets, values)| values)
                                )
                            });

                    let all_frames =
                        re_query2::range_zip_1x1(all_scalars_indexed, all_marker_shapes)
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
                let series_name = results
                    .get_optional_chunks(&Name::name())
                    .iter()
                    .find(|chunk| !chunk.is_empty())
                    .and_then(|chunk| chunk.component_mono::<Name>(0))
                    .unwrap_or_else(|| self.fallback_for(&query_ctx));

                // Now convert the `PlotPoints` into `Vec<PlotSeries>`
                points_to_series(
                    &data_result.entity_path,
                    time_per_pixel,
                    points,
                    ctx.recording_store(),
                    view_query,
                    &series_name,
                    // Aggregation for points is not supported.
                    re_types::components::AggregationPolicy::Off,
                    &mut self.all_series,
                );
            }
        }

        Ok(())
    }
}
