use itertools::Itertools as _;

use re_query_cache2::{PromiseResult, QueryError};
use re_types::{
    archetypes::{self, SeriesPoint},
    components::{Color, MarkerShape, MarkerSize, Name, Scalar, StrokeWidth},
    Archetype as _, ComponentNameSet, Loggable,
};
use re_viewer_context::{
    AnnotationMap, DefaultColor, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::overrides::initial_override_color;
use crate::util::{
    determine_plot_bounds_and_time_per_pixel, determine_time_range, points_to_series,
};
use crate::ScatterAttrs;
use crate::{PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

/// The system for rendering [`SeriesPoint`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesPointSystem {
    pub annotation_map: AnnotationMap,
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
        query_info.queried.insert(StrokeWidth::name());
        query_info.indicators = std::iter::once(SeriesPoint::indicator().name()).collect();
        query_info
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.annotation_map.load(
            ctx,
            &query.latest_at_query(),
            query
                .iter_visible_data_results(ctx, Self::identifier())
                .map(|data| &data.entity_path),
        );

        match self.load_scalars(ctx, query) {
            Ok(_) | Err(QueryError::PrimaryNotFound(_)) => Ok(Vec::new()),
            Err(err) => Err(err.into()),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn initial_override_value(
        &self,
        _ctx: &ViewerContext<'_>,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
        entity_path: &re_log_types::EntityPath,
        component: &re_types::ComponentName,
    ) -> Option<re_log_types::DataCell> {
        if *component == Color::name() {
            Some([initial_override_color(entity_path)].into())
        } else if *component == MarkerSize::name() {
            Some([MarkerSize(DEFAULT_MARKER_SIZE)].into())
        } else {
            None
        }
    }
}

impl SeriesPointSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let store = ctx.recording_store();
        let query_caches2 = ctx.recording().query_caches2();
        let resolver = ctx.recording().resolver();

        let (plot_bounds, time_per_pixel) = determine_plot_bounds_and_time_per_pixel(ctx, query);

        // TODO(cmc): this should be thread-pooled in case there are a gazillon series in the same plot…
        for data_result in query.iter_visible_data_results(ctx, Self::identifier()) {
            let annotations = self.annotation_map.find(&data_result.entity_path);
            let annotation_info = annotations
                .resolved_class_description(None)
                .annotation_info();
            let default_color = DefaultColor::EntityPath(&data_result.entity_path);

            let override_color = data_result
                .lookup_override::<Color>(ctx)
                .map(|c| c.to_array());
            let override_series_name = data_result.lookup_override::<Name>(ctx).map(|t| t.0);
            let override_marker_size = data_result.lookup_override::<MarkerSize>(ctx).map(|r| r.0);
            let override_marker_shape = data_result.lookup_override::<MarkerShape>(ctx);

            // All the default values for a `PlotPoint`, accounting for both overrides and default
            // values.
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    label: None,
                    color: annotation_info.color(override_color, default_color),
                    marker_size: override_marker_size.unwrap_or(DEFAULT_MARKER_SIZE),
                    kind: PlotSeriesKind::Scatter(ScatterAttrs {
                        marker: override_marker_shape.unwrap_or_default(),
                    }),
                },
            };

            let mut points;

            let time_range = determine_time_range(
                ctx,
                query,
                data_result,
                plot_bounds,
                ctx.app_options.experimental_plot_query_clamping,
            );

            {
                re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

                let entity_path = &data_result.entity_path;
                let query = re_data_store::RangeQuery::new(query.timeline, time_range);

                let results = query_caches2.range(
                    store,
                    &query,
                    entity_path,
                    [Scalar::name(), Color::name(), StrokeWidth::name()],
                );

                let all_scalars = results
                    .get_required(Scalar::name())?
                    .to_dense::<Scalar>(resolver);
                let all_scalars_entry_range = all_scalars.entry_range(query.range());

                if !matches!(
                    all_scalars.status(query.range()),
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

                // Fill in colors -- if available _and_ not overridden.
                if override_color.is_none() {
                    if let Some(all_colors) = results.get(Color::name()) {
                        let all_colors = all_colors.to_dense::<Color>(resolver);

                        if !matches!(
                            all_colors.status(query.range()),
                            (PromiseResult::Ready(()), PromiseResult::Ready(()))
                        ) {
                            // TODO(#5607): what should happen if the promise is still pending?
                        }

                        let all_scalars_indexed = all_scalars
                            .range_indices(all_scalars_entry_range.clone())
                            .map(|index| (index, ()));

                        let all_frames = re_query_cache2::range_zip_1x1(
                            all_scalars_indexed,
                            all_colors.range_indexed(query.range()),
                        )
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
                }

                // Fill in marker sizes -- if available _and_ not overridden.
                if override_marker_size.is_none() {
                    if let Some(all_marker_sizes) = results.get(MarkerSize::name()) {
                        let all_marker_sizes = all_marker_sizes.to_dense::<MarkerSize>(resolver);

                        if !matches!(
                            all_marker_sizes.status(query.range()),
                            (PromiseResult::Ready(()), PromiseResult::Ready(()))
                        ) {
                            // TODO(#5607): what should happen if the promise is still pending?
                        }

                        let all_scalars_indexed = all_scalars
                            .range_indices(all_scalars_entry_range.clone())
                            .map(|index| (index, ()));

                        let all_frames = re_query_cache2::range_zip_1x1(
                            all_scalars_indexed,
                            all_marker_sizes.range_indexed(query.range()),
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
                }

                // Fill in marker shapes -- if available _and_ not overridden.
                if override_marker_shape.is_none() {
                    if let Some(all_marker_shapes) = results.get(MarkerShape::name()) {
                        let all_marker_shapes = all_marker_shapes.to_dense::<MarkerShape>(resolver);

                        if !matches!(
                            all_marker_shapes.status(query.range()),
                            (PromiseResult::Ready(()), PromiseResult::Ready(()))
                        ) {
                            // TODO(#5607): what should happen if the promise is still pending?
                        }

                        let all_scalars_indexed = all_scalars
                            .range_indices(all_scalars_entry_range.clone())
                            .map(|index| (index, ()));

                        let all_frames = re_query_cache2::range_zip_1x1(
                            all_scalars_indexed,
                            all_marker_shapes.range_indexed(query.range()),
                        )
                        .enumerate();

                        for (i, (_index, _scalars, marker_shapes)) in all_frames {
                            if let Some(marker) = marker_shapes
                                .and_then(|marker_shapes| marker_shapes.first().copied())
                            {
                                points[i].attrs.kind =
                                    PlotSeriesKind::Scatter(ScatterAttrs { marker });
                            }
                        }
                    }
                }
            }

            // Check for an explicit label if any.
            // We're using a separate latest-at query for this since the semantics for labels changing over time are a
            // a bit unclear.
            // Sidestepping the cache here shouldn't be a problem since we do so only once per entity.
            let series_name = if let Some(override_name) = override_series_name {
                Some(override_name)
            } else {
                // TODO(#5607): what should happen if the promise is still pending?
                ctx.recording()
                    .latest_at_component::<Name>(&data_result.entity_path, &ctx.current_query())
                    .map(|name| name.value.0)
            };

            // Now convert the `PlotPoints` into `Vec<PlotSeries>`
            points_to_series(
                data_result,
                time_per_pixel,
                points,
                store,
                query,
                series_name,
                &mut self.all_series,
            );
        }

        Ok(())
    }
}
