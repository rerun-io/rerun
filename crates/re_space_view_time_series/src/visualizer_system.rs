use re_data_store::TimeRange;
use re_log_types::{EntityPath, StoreKind, TimeInt};
use re_query_cache::{MaybeCachedComponentData, QueryError};
use re_types::{
    archetypes::TimeSeriesScalar,
    components::{Color, Radius, Scalar, ScalarScattering, Text},
    Component, Loggable,
};
use re_viewer_context::{
    external::re_entity_db::TimeSeriesAggregator, AnnotationMap, DefaultColor,
    IdentifiedViewSystem, ResolvedAnnotationInfo, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    aggregation::{AverageAggregator, MinMaxAggregator},
    PlotPoint, PlotPointAttrs, PlotSeries, PlotSeriesKind,
};

/// A scene for a time series plot, with everything needed to render it.
#[derive(Default, Debug)]
pub struct TimeSeriesSystem {
    pub annotation_map: AnnotationMap,
    pub lines: Vec<PlotSeries>,

    /// Earliest time an entity was recorded at on the current timeline.
    pub min_time: Option<i64>,

    /// What kind of aggregation was used to compute the graph?
    pub aggregator: TimeSeriesAggregator,

    /// `1.0` for raw data.
    pub aggregation_factor: f64,
}

impl IdentifiedViewSystem for TimeSeriesSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TimeSeries".into()
    }
}

impl VisualizerSystem for TimeSeriesSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<TimeSeriesScalar>()
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
            let default_color = DefaultColor::EntityPath(entity_path);

            let annotation_info = ResolvedAnnotationInfo::default();

            let color = annotation_info.color(None, default_color);

            let [r, g, b, a] = color.to_array();

            Some([Color::from_unmultiplied_rgba(r, g, b, a)].into())
        } else {
            None
        }
    }
}

impl TimeSeriesSystem {
    fn load_scalars(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let query_caches = ctx.entity_db.query_caches();
        let store = ctx.entity_db.store();

        let egui_ctx = &ctx.re_ui.egui_ctx;

        let plot_mem = egui_plot::PlotMemory::load(egui_ctx, crate::plot_id(query.space_view_id));
        let plot_bounds = plot_mem.as_ref().map(|mem| *mem.bounds());
        // What's the delta in value of X across two adjacent UI points?
        // I.e. think of GLSL's `dpdx()`.
        let plot_value_delta = plot_mem.as_ref().map_or(1.0, |mem| {
            1.0 / mem.transform().dpos_dvalue_x().max(f64::EPSILON)
        });

        // TODO(cmc): this should be thread-pooled in case there are a gazillon series in the same plot…
        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let mut points = Vec::new();

            let visible_history = match query.timeline.typ() {
                re_log_types::TimeType::Time => {
                    data_result.accumulated_properties().visible_history.nanos
                }
                re_log_types::TimeType::Sequence => {
                    data_result
                        .accumulated_properties()
                        .visible_history
                        .sequences
                }
            };

            let (mut from, mut to) = if data_result.accumulated_properties().visible_history.enabled
            {
                (
                    visible_history.from(query.latest_at),
                    visible_history.to(query.latest_at),
                )
            } else {
                (TimeInt::MIN, TimeInt::MAX)
            };

            // TODO(cmc): We would love to reduce the query to match the actual plot bounds, but because
            // the plot widget handles zoom after we provide it with data for the current frame,
            // this results in an extremely jarring frame delay.
            // Just try it out and you'll see what I mean.
            if false {
                if let Some(plot_bounds) = plot_bounds {
                    from =
                        TimeInt::max(from, (plot_bounds.range_x().start().floor() as i64).into());
                    to = TimeInt::min(to, (plot_bounds.range_x().end().ceil() as i64).into());
                }
            }

            {
                re_tracing::profile_scope!("primary", &data_result.entity_path.to_string());

                let annotations = self.annotation_map.find(&data_result.entity_path);
                let annotation_info = annotations
                    .resolved_class_description(None)
                    .annotation_info();
                let default_color = DefaultColor::EntityPath(&data_result.entity_path);

                let override_color = lookup_override::<Color>(data_result, ctx).map(|c| {
                    let arr = c.to_array();
                    egui::Color32::from_rgba_unmultiplied(arr[0], arr[1], arr[2], arr[3])
                });

                let override_label =
                    lookup_override::<Text>(data_result, ctx).map(|t| t.to_string());

                let override_scattered =
                    lookup_override::<ScalarScattering>(data_result, ctx).map(|s| s.0);

                let override_radius = lookup_override::<Radius>(data_result, ctx).map(|r| r.0);

                let query =
                    re_data_store::RangeQuery::new(query.timeline, TimeRange::new(from, to));

                query_caches.query_archetype_pov1_comp4::<
                    TimeSeriesScalar,
                    Scalar,
                    ScalarScattering,
                    Color,
                    Radius,
                    Text,
                    _,
                >(
                    ctx.app_options.experimental_primary_caching_range,
                    store,
                    &query.clone().into(),
                    &data_result.entity_path,
                    |((time, _row_id), _, scalars, scatterings, colors, radii, labels)| {
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
                                    radius: 0.0,
                                    kind: PlotSeriesKind::Clear,
                                },
                            });
                            return;
                        }

                        for (scalar, scattered, color, radius, label) in itertools::izip!(
                            scalars.iter(),
                            MaybeCachedComponentData::iter_or_repeat_opt(&scatterings, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&colors, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&radii, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&labels, scalars.len()),
                        ) {
                            let color = override_color.unwrap_or_else(|| {
                                annotation_info.color(color.map(|c| c.to_array()), default_color)
                            });
                            let label = override_label.clone().or_else(|| {
                                annotation_info.label(label.as_ref().map(|l| l.as_str()))
                            });
                            let scattered = override_scattered
                                .unwrap_or_else(|| scattered.map_or(false, |s| s.0));
                            let radius = override_radius
                                .unwrap_or_else(|| radius.map_or(DEFAULT_RADIUS, |r| r.0));

                            let kind = if scattered {
                                PlotSeriesKind::Scatter
                            } else {
                                PlotSeriesKind::Continuous
                            };

                            const DEFAULT_RADIUS: f32 = 0.75;

                            points.push(PlotPoint {
                                time: time.as_i64(),
                                value: scalar.0,
                                attrs: PlotPointAttrs {
                                    label,
                                    color,
                                    radius,
                                    kind,
                                },
                            });
                        }
                    },
                )?;
            }

            if points.is_empty() {
                continue;
            }

            let (aggregation_factor, points) = {
                let aggregator = data_result
                    .accumulated_properties()
                    .time_series_aggregator
                    .get();

                // So it can be displayed in the UI by the SpaceViewClass.
                self.aggregator = *aggregator;

                let aggregation_factor = plot_value_delta;
                let num_points_before = points.len() as f64;

                let points = if aggregation_factor > 2.0 {
                    re_tracing::profile_scope!("aggregate", aggregator.to_string());

                    #[allow(clippy::match_same_arms)] // readability
                    match aggregator {
                        TimeSeriesAggregator::Off => points,
                        TimeSeriesAggregator::Average => {
                            AverageAggregator::aggregate(aggregation_factor, &points)
                        }
                        TimeSeriesAggregator::Min => {
                            MinMaxAggregator::Min.aggregate(aggregation_factor, &points)
                        }
                        TimeSeriesAggregator::Max => {
                            MinMaxAggregator::Max.aggregate(aggregation_factor, &points)
                        }
                        TimeSeriesAggregator::MinMax => {
                            MinMaxAggregator::MinMax.aggregate(aggregation_factor, &points)
                        }
                        TimeSeriesAggregator::MinMaxAverage => {
                            MinMaxAggregator::MinMaxAverage.aggregate(aggregation_factor, &points)
                        }
                    }
                } else {
                    points
                };

                let num_points_after = points.len() as f64;
                let actual_aggregation_factor = num_points_before / num_points_after;

                re_log::trace!(
                    id = %query.space_view_id,
                    plot_value_delta,
                    ?aggregator,
                    aggregation_factor,
                    num_points_before,
                    num_points_after,
                    actual_aggregation_factor,
                );

                (actual_aggregation_factor, points)
            };

            // So it can be displayed in the UI by the SpaceViewClass.
            self.aggregation_factor = aggregation_factor;

            re_tracing::profile_scope!("secondary", &data_result.entity_path.to_string());

            let min_time = store
                .entity_min_time(&query.timeline, &data_result.entity_path)
                .map_or(points.first().map_or(0, |p| p.time), |time| time.as_i64());

            self.min_time = Some(self.min_time.map_or(min_time, |time| time.min(min_time)));

            // If all points within a line share the label (and it isn't `None`), then we use it
            // as the whole line label for the plot legend.
            // Otherwise, we just use the entity path as-is.
            let same_label = |points: &[PlotPoint]| -> Option<String> {
                let label = points[0].attrs.label.as_ref()?;
                (points.iter().all(|p| p.attrs.label.as_ref() == Some(label)))
                    .then(|| label.clone())
            };
            let line_label =
                same_label(&points).unwrap_or_else(|| data_result.entity_path.to_string());

            if points.len() == 1 {
                self.lines.push(PlotSeries {
                    label: line_label,
                    color: points[0].attrs.color,
                    width: 2.0 * points[0].attrs.radius,
                    kind: PlotSeriesKind::Scatter,
                    points: vec![(points[0].time, points[0].value)],
                    entity_path: data_result.entity_path.clone(),
                });
            } else {
                self.add_line_segments(&line_label, points, &data_result.entity_path);
            }
        }

        Ok(())
    }

    // We have a bunch of raw points, and now we need to group them into actual line
    // segments.
    // A line segment is a continuous run of points with identical attributes: each time
    // we notice a change in attributes, we need a new line segment.
    #[inline(never)] // Better callstacks on crashes
    fn add_line_segments(
        &mut self,
        line_label: &str,
        points: Vec<PlotPoint>,
        entity_path: &EntityPath,
    ) {
        re_tracing::profile_function!();

        let num_points = points.len();
        let mut attrs = points[0].attrs.clone();
        let mut line: PlotSeries = PlotSeries {
            label: line_label.to_owned(),
            color: attrs.color,
            width: 2.0 * attrs.radius,
            points: Vec::with_capacity(num_points),
            kind: attrs.kind,
            entity_path: entity_path.clone(),
        };

        for (i, p) in points.into_iter().enumerate() {
            if p.attrs == attrs {
                // Same attributes, just add to the current line segment.

                line.points.push((p.time, p.value));
            } else {
                // Attributes changed since last point, break up the current run into a
                // line segment, and start the next one.

                attrs = p.attrs;
                let prev_line = std::mem::replace(
                    &mut line,
                    PlotSeries {
                        label: line_label.to_owned(),
                        color: attrs.color,
                        width: 2.0 * attrs.radius,
                        kind: attrs.kind,
                        points: Vec::with_capacity(num_points - i),
                        entity_path: entity_path.clone(),
                    },
                );

                let cur_continuous = matches!(attrs.kind, PlotSeriesKind::Continuous);
                let prev_continuous = matches!(prev_line.kind, PlotSeriesKind::Continuous);

                let prev_point = *prev_line.points.last().unwrap();
                self.lines.push(prev_line);

                // If the previous point was continuous and the current point is continuous
                // too, then we want the 2 segments to appear continuous even though they
                // are actually split from a data standpoint.
                if cur_continuous && prev_continuous {
                    line.points.push(prev_point);
                }

                // Add the point that triggered the split to the new segment.
                line.points.push((p.time, p.value));
            }
        }

        if !line.points.is_empty() {
            self.lines.push(line);
        }
    }
}

fn lookup_override<C: Component>(
    data_result: &re_viewer_context::DataResult,
    ctx: &ViewerContext<'_>,
) -> Option<C> {
    data_result
        .property_overrides
        .as_ref()
        .and_then(|p| p.component_overrides.get(&C::name()))
        .and_then(|(store_kind, path)| match store_kind {
            StoreKind::Blueprint => ctx
                .store_context
                .blueprint
                .store()
                .query_latest_component::<C>(path, ctx.blueprint_query),
            StoreKind::Recording => ctx
                .entity_db
                .store()
                .query_latest_component::<C>(path, &ctx.current_query()),
        })
        .map(|c| c.value)
}

// ---
