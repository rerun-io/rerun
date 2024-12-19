use itertools::Itertools as _;

use re_types::{
    archetypes::{self, SeriesPoint},
    components::{Color, MarkerShape, MarkerSize, Name, Scalar},
    external::arrow::datatypes::DataType as ArrowDatatype,
    Archetype as _, Component as _, Loggable as _,
};
use re_view::range_with_blueprint_resolved_data;
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, TypedComponentFallbackProvider,
    ViewContext, ViewQuery, ViewStateExt as _, ViewSystemExecutionError, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    util::{determine_time_per_pixel, determine_time_range, points_to_series},
    view_class::TimeSeriesViewState,
    PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind, ScatterAttrs,
};

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
        query_info.queried.extend(
            SeriesPoint::all_components()
                .iter()
                .map(|descr| descr.component_name),
        );

        use re_types::ComponentBatch as _;
        query_info.indicators = std::iter::once(SeriesPoint::indicator().name()).collect();

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

re_viewer_context::impl_component_fallback_provider!(SeriesPointSystem => [Color, MarkerSize, Name]);

impl SeriesPointSystem {
    fn load_scalars(&mut self, ctx: &ViewContext<'_>, query: &ViewQuery<'_>) {
        re_tracing::profile_function!();

        let plot_mem =
            egui_plot::PlotMemory::load(ctx.viewer_ctx.egui_ctx, crate::plot_id(query.view_id));
        let time_per_pixel = determine_time_per_pixel(ctx.viewer_ctx, plot_mem.as_ref());

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
                // NOTE: arguably, the `MarkerSize` value should be twice the `radius_ui`. We do
                // stick to the semantics of `MarkerSize` == radius for backward compatibility and
                // because markers need a decent radius value to be at all legible.
                radius_ui: **fallback_size,
                kind: PlotSeriesKind::Scatter(ScatterAttrs {
                    marker: fallback_shape,
                }),
            },
        };

        let mut points;

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
                    Color::name(),
                    MarkerShape::name(),
                    MarkerSize::name(),
                    Name::name(),
                    Scalar::name(),
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
                let mut i = 0;
                all_scalar_chunks
                        .iter()
                        .flat_map(|chunk| chunk.iter_primitive::<f64>(&Scalar::name()))
                        .for_each(|values| {
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

                            i += 1;
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

                {
                    let all_color_chunks = results.get_optional_chunks(&Color::name());

                    if all_color_chunks.len() == 1 && all_color_chunks[0].is_static() {
                        re_tracing::profile_scope!("override/default fast path");

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

            // Fill in marker sizes
            {
                re_tracing::profile_scope!("fill marker sizes");

                debug_assert_eq!(MarkerSize::arrow_datatype(), ArrowDatatype::Float32);

                {
                    let all_marker_size_chunks = results.get_optional_chunks(&MarkerSize::name());

                    if all_marker_size_chunks.len() == 1 && all_marker_size_chunks[0].is_static() {
                        re_tracing::profile_scope!("override/default fast path");

                        let marker_size = all_marker_size_chunks[0]
                            .iter_primitive::<f32>(&MarkerSize::name())
                            .next()
                            .and_then(|marker_sizes| marker_sizes.first().copied());

                        if let Some(marker_size) = marker_size {
                            points
                                .iter_mut()
                                // `marker_size` is a radius, see NOTE above
                                .for_each(|p| p.attrs.radius_ui = marker_size);
                        }
                    } else {
                        re_tracing::profile_scope!("standard path");

                        let all_marker_sizes = all_marker_size_chunks.iter().flat_map(|chunk| {
                            itertools::izip!(
                                chunk
                                    .iter_component_indices(&query.timeline(), &MarkerSize::name()),
                                chunk.iter_primitive::<f32>(&MarkerSize::name())
                            )
                        });

                        let all_frames =
                            re_query::range_zip_1x1(all_scalars_indices(), all_marker_sizes)
                                .enumerate();

                        all_frames.for_each(|(i, (_index, _scalars, marker_sizes))| {
                            if let Some(marker_size) =
                                marker_sizes.and_then(|marker_sizes| marker_sizes.first().copied())
                            {
                                // `marker_size` is a radius, see NOTE above
                                points[i].attrs.radius_ui = marker_size;
                            }
                        });
                    }
                }
            }

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

                        let marker_shape = all_marker_shapes_chunks[0]
                            .iter_component::<MarkerShape>()
                            .next()
                            .and_then(|marker_shapes| marker_shapes.first().copied());

                        if let Some(marker_shape) = marker_shape {
                            for p in &mut points {
                                p.attrs.kind = PlotSeriesKind::Scatter(ScatterAttrs {
                                    marker: marker_shape,
                                });
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
                                        &query.timeline(),
                                        &MarkerShape::name(),
                                    )
                                });
                            itertools::izip!(all_marker_shapes_indices, all_marker_shapes)
                        };

                        let all_frames = re_query::range_zip_1x1(
                            all_scalars_indices(),
                            all_marker_shapes_indexed,
                        )
                        .enumerate();

                        all_frames.for_each(|(i, (_index, _scalars, marker_shapes))| {
                            if let Some(marker_shape) = marker_shapes
                                .and_then(|marker_shapes| marker_shapes.first().copied())
                            {
                                points[i].attrs.kind = PlotSeriesKind::Scatter(ScatterAttrs {
                                    marker: marker_shape,
                                });
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
                .unwrap_or_else(|| self.fallback_for(&query_ctx));

            // Now convert the `PlotPoints` into `Vec<PlotSeries>`
            points_to_series(
                &data_result.entity_path,
                time_per_pixel,
                points,
                ctx.recording_engine().store(),
                view_query,
                series_name.into(),
                // Aggregation for points is not supported.
                re_types::components::AggregationPolicy::Off,
                all_series,
            );
        }
    }
}
