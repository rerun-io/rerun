use re_query_cache::{MaybeCachedComponentData, QueryError};
use re_types::archetypes;
use re_types::{
    archetypes::SeriesLine,
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
use crate::{overrides::lookup_override, PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind};

/// The system for rendering [`SeriesLine`] archetypes.
#[derive(Default, Debug)]
pub struct SeriesLineSystem {
    pub annotation_map: AnnotationMap,
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
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.annotation_map.load(
            ctx,
            &query.latest_at_query(),
            query
                .iter_visible_data_results(Self::identifier())
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
        } else if *component == StrokeWidth::name() {
            Some([StrokeWidth(DEFAULT_STROKE_WIDTH)].into())
        } else {
            None
        }
    }
}

impl SeriesLineSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let query_caches = ctx.entity_db.query_caches();
        let store = ctx.entity_db.store();

        let (plot_bounds, time_per_pixel) = determine_plot_bounds_and_time_per_pixel(ctx, query);

        // TODO(cmc): this should be thread-pooled in case there are a gazillon series in the same plot…
        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let annotations = self.annotation_map.find(&data_result.entity_path);
            let annotation_info = annotations
                .resolved_class_description(None)
                .annotation_info();
            let default_color = DefaultColor::EntityPath(&data_result.entity_path);

            let override_color = lookup_override::<Color>(data_result, ctx).map(|c| c.to_array());
            let override_series_name = lookup_override::<Name>(data_result, ctx).map(|t| t.0);
            let override_stroke_width =
                lookup_override::<StrokeWidth>(data_result, ctx).map(|r| r.0);

            // All the default values for a `PlotPoint`, accounting for both overrides and default
            // values.
            let default_point = PlotPoint {
                time: 0,
                value: 0.0,
                attrs: PlotPointAttrs {
                    label: None,
                    color: annotation_info.color(override_color, default_color),
                    marker_size: override_stroke_width.unwrap_or(DEFAULT_STROKE_WIDTH),
                    kind: PlotSeriesKind::Continuous,
                },
            };

            let mut points = Vec::new();

            let time_range = determine_time_range(
                query,
                data_result,
                plot_bounds,
                ctx.app_options.experimental_plot_query_clamping,
            );

            {
                re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());
                let query = re_data_store::RangeQuery::new(query.timeline, time_range);

                // TODO(jleibs): need to do a "joined" archetype query
                // The `Scalar` archetype queries for `MarkerSize` in the point visualizer,
                // and so it must do so here also.
                // See https://github.com/rerun-io/rerun/pull/5029
                query_caches
                    .query_archetype_pov1_comp4::<archetypes::Scalar, Scalar, Color, StrokeWidth, MarkerSize, MarkerShape, _>(
                        ctx.app_options.experimental_primary_caching_range,
                        store,
                        &query.clone().into(),
                        &data_result.entity_path,
                        // The `Scalar` archetype queries for `MarkerShape` in the point visualizer,
                        // and so it must do so here also.
                        // See https://github.com/rerun-io/rerun/pull/5029
                        |((time, _row_id), _, scalars, colors, stroke_widths, _, _)| {
                            let Some(time) = time else {
                                return;
                            }; // scalars cannot be timeless

                            // This is a clear: we want to split the chart.
                            if scalars.is_empty() {
                                points.push(PlotPoint {
                                    time: time.as_i64(),
                                    value: 0.0,
                                    attrs: PlotPointAttrs {
                                        label: None,
                                        color: egui::Color32::BLACK,
                                        marker_size: 0.0,
                                        kind: PlotSeriesKind::Clear,
                                    },
                                });
                                return;
                            }

                            for (scalar, color, stroke_width) in itertools::izip!(
                                scalars.iter(),
                                MaybeCachedComponentData::iter_or_repeat_opt(&colors, scalars.len()),
                                MaybeCachedComponentData::iter_or_repeat_opt(&stroke_widths, scalars.len()),
                            ) {
                                let mut point = PlotPoint {
                                    time: time.as_i64(),
                                    value: scalar.0,
                                    ..default_point.clone()
                                };

                                // Make it as clear as possible to the optimizer that some parameters
                                // go completely unused as soon as overrides have been defined.

                                if override_color.is_none() {
                                    if let Some(color) = color.map(|c| {
                                        let [r,g,b,a] = c.to_array();
                                        if a == 255 {
                                            // Common-case optimization
                                            re_renderer::Color32::from_rgb(r, g, b)
                                        } else {
                                            re_renderer::Color32::from_rgba_unmultiplied(r, g, b, a)
                                        }
                                    }) {
                                        point.attrs.color = color;
                                    }
                                }

                                if override_stroke_width.is_none() {
                                    if let Some(stroke_width) = stroke_width.map(|r| r.0) {
                                        point.attrs.marker_size = stroke_width;
                                    }
                                }

                                points.push(point);
                            }
                        },
                    )?;
            }

            // Check for an explicit label if any.
            // We're using a separate latest-at query for this since the semantics for labels changing over time are a
            // a bit unclear.
            // Sidestepping the cache here shouldn't be a problem since we do so only once per entity.
            let series_name = if let Some(override_name) = override_series_name {
                Some(override_name)
            } else {
                ctx.entity_db
                    .store()
                    .query_latest_component::<Name>(&data_result.entity_path, &ctx.current_query())
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
