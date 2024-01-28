use re_data_store::TimeRange;
use re_log_types::TimeInt;
use re_query_cache::{MaybeCachedComponentData, QueryError};
use re_types::{
    archetypes::TimeSeriesScalar,
    components::{Color, Radius, Scalar, ScalarScattering, Text},
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    external::re_entity_db::TimeSeriesAggregator, AnnotationMap, DefaultColor,
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
    VisualizerSystem,
};

// ---

#[derive(Clone, Debug)]
pub struct PlotPointAttrs {
    pub label: Option<String>,
    pub color: egui::Color32,
    pub radius: f32,
    pub scattered: bool,
}

impl PartialEq for PlotPointAttrs {
    fn eq(&self, rhs: &Self) -> bool {
        let Self {
            label,
            color,
            radius,
            scattered,
        } = self;
        label.eq(&rhs.label)
            && color.eq(&rhs.color)
            && radius.total_cmp(&rhs.radius).is_eq()
            && scattered.eq(&rhs.scattered)
    }
}

impl Eq for PlotPointAttrs {}

#[derive(Clone, Debug)]
struct PlotPoint {
    time: i64,
    value: f64,
    attrs: PlotPointAttrs,
}

#[derive(Clone, Copy, Debug)]
pub enum PlotSeriesKind {
    Continuous,
    Scatter,
}

#[derive(Clone, Debug)]
pub struct PlotSeries {
    pub label: String,
    pub color: egui::Color32,
    pub width: f32,
    pub kind: PlotSeriesKind,
    pub points: Vec<(i64, f64)>,
}

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
    fn required_components(&self) -> ComponentNameSet {
        TimeSeriesScalar::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(TimeSeriesScalar::indicator().name()).collect()
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

                        for (scalar, scattered, color, radius, label) in itertools::izip!(
                            scalars.iter(),
                            MaybeCachedComponentData::iter_or_repeat_opt(&scatterings, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&colors, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&radii, scalars.len()),
                            MaybeCachedComponentData::iter_or_repeat_opt(&labels, scalars.len()),
                        ) {
                            let color =
                                annotation_info.color(color.map(|c| c.to_array()), default_color);
                            let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));

                            const DEFAULT_RADIUS: f32 = 0.75;

                            points.push(PlotPoint {
                                time: time.as_i64(),
                                value: scalar.0,
                                attrs: PlotPointAttrs {
                                    label,
                                    color,
                                    radius: radius.map_or(DEFAULT_RADIUS, |r| r.0),
                                    scattered: scattered.map_or(false, |s| s.0),
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
                });
            } else {
                self.add_line_segments(&line_label, points);
            }
        }

        Ok(())
    }

    // We have a bunch of raw points, and now we need to group them into actual line
    // segments.
    // A line segment is a continuous run of points with identical attributes: each time
    // we notice a change in attributes, we need a new line segment.
    #[inline(never)] // Better callstacks on crashes
    fn add_line_segments(&mut self, line_label: &str, points: Vec<PlotPoint>) {
        re_tracing::profile_function!();

        let num_points = points.len();
        let mut attrs = points[0].attrs.clone();
        let mut line: PlotSeries = PlotSeries {
            label: line_label.to_owned(),
            color: attrs.color,
            width: 2.0 * attrs.radius,
            kind: if attrs.scattered {
                PlotSeriesKind::Scatter
            } else {
                PlotSeriesKind::Continuous
            },
            points: Vec::with_capacity(num_points),
        };

        for (i, p) in points.into_iter().enumerate() {
            if p.attrs == attrs {
                // Same attributes, just add to the current line segment.

                line.points.push((p.time, p.value));
            } else {
                // Attributes changed since last point, break up the current run into a
                // line segment, and start the next one.

                attrs = p.attrs.clone();
                let kind = if attrs.scattered {
                    PlotSeriesKind::Scatter
                } else {
                    PlotSeriesKind::Continuous
                };

                let prev_line = std::mem::replace(
                    &mut line,
                    PlotSeries {
                        label: line_label.to_owned(),
                        color: attrs.color,
                        width: 2.0 * attrs.radius,
                        kind,
                        points: Vec::with_capacity(num_points - i),
                    },
                );
                let prev_point = *prev_line.points.last().unwrap();
                self.lines.push(prev_line);

                // If the previous point was continuous and the current point is continuous
                // too, then we want the 2 segments to appear continuous even though they
                // are actually split from a data standpoint.
                let cur_continuous = matches!(kind, PlotSeriesKind::Continuous);
                let prev_continuous = matches!(kind, PlotSeriesKind::Continuous);
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

// ---

/// Implements aggregation behavior corresponding to [`TimeSeriesAggregator::Average`].
struct AverageAggregator;

impl AverageAggregator {
    #[inline]
    fn aggregate(aggregation_factor: f64, points: &[PlotPoint]) -> Vec<PlotPoint> {
        let min_time = points.first().map_or(i64::MIN, |p| p.time);
        let max_time = points.last().map_or(i64::MAX, |p| p.time);

        let mut aggregated =
            Vec::with_capacity((points.len() as f64 / aggregation_factor) as usize);

        // NOTE: `floor()` since we handle fractional tails separately.
        let window_size = usize::max(1, aggregation_factor.floor() as usize);
        let aggregation_factor_fract = aggregation_factor.fract();

        let mut i = 0;
        while i < points.len() {
            let mut j = 0;

            let mut ratio = 1.0;
            let mut acc = points[i + j].clone();
            j += 1;

            while j < window_size
                && i + j < points.len()
                && are_aggregatable(&points[i], &points[i + j], window_size)
            {
                let point = &points[i + j];

                acc.value += point.value;
                acc.attrs.radius += point.attrs.radius;

                ratio += 1.0;
                j += 1;
            }

            // Do a weighted average for the fractional tail.
            if aggregation_factor_fract > 0.0
                && i + j < points.len()
                && are_aggregatable(&points[i], &points[i + j], window_size)
            {
                let point = &points[i + j];

                let w = aggregation_factor_fract;
                acc.value += point.value * w;
                acc.attrs.radius += (point.attrs.radius as f64 * w) as f32;

                ratio += aggregation_factor_fract;
                j += 1;
            }

            acc.value /= ratio;
            acc.attrs.radius = (acc.attrs.radius as f64 / ratio) as _;

            aggregated.push(acc);

            i += j;
        }

        // Force align the start and end timestamps to prevent jarring visual glitches.
        if let Some(p) = aggregated.first_mut() {
            p.time = min_time;
        }
        if let Some(p) = aggregated.last_mut() {
            p.time = max_time;
        }

        aggregated
    }
}

/// Implements aggregation behaviors corresponding to [`TimeSeriesAggregator::Max`],
/// [`TimeSeriesAggregator::Min`], [`TimeSeriesAggregator::MinMax`] and
/// [`TimeSeriesAggregator::MinMaxAverage`], .
enum MinMaxAggregator {
    /// Keep only the maximum values in the range.
    Max,

    /// Keep only the minimum values in the range.
    Min,

    /// Keep both the minimum and maximum values in the range.
    ///
    /// This will yield two aggregated points instead of one, effectively creating a vertical line.
    MinMax,

    /// Find both the minimum and maximum values in the range, then use the average of those.
    MinMaxAverage,
}

impl MinMaxAggregator {
    #[inline]
    fn aggregate(&self, aggregation_factor: f64, points: &[PlotPoint]) -> Vec<PlotPoint> {
        let min_time = points.first().map_or(i64::MIN, |p| p.time);
        let max_time = points.last().map_or(i64::MAX, |p| p.time);

        let capacity = (points.len() as f64 / aggregation_factor) as usize;
        let mut aggregated = match self {
            MinMaxAggregator::MinMax => Vec::with_capacity(capacity * 2),
            _ => Vec::with_capacity(capacity),
        };

        // NOTE: `round()` since this can only handle discrete window sizes.
        let window_size = usize::max(1, aggregation_factor.round() as usize);

        let mut i = 0;
        while i < points.len() {
            let mut j = 0;

            let mut acc_min = points[i + j].clone();
            let mut acc_max = points[i + j].clone();
            j += 1;

            while j < window_size
                && i + j < points.len()
                && are_aggregatable(&points[i], &points[i + j], window_size)
            {
                let point = &points[i + j];

                match self {
                    MinMaxAggregator::MinMax | MinMaxAggregator::MinMaxAverage => {
                        acc_min.value = f64::min(acc_min.value, point.value);
                        acc_min.attrs.radius = f32::min(acc_min.attrs.radius, point.attrs.radius);
                        acc_max.value = f64::max(acc_max.value, point.value);
                        acc_max.attrs.radius = f32::max(acc_max.attrs.radius, point.attrs.radius);
                    }
                    MinMaxAggregator::Min => {
                        acc_min.value = f64::min(acc_min.value, point.value);
                        acc_min.attrs.radius = f32::min(acc_min.attrs.radius, point.attrs.radius);
                    }
                    MinMaxAggregator::Max => {
                        acc_max.value = f64::max(acc_max.value, point.value);
                        acc_max.attrs.radius = f32::max(acc_max.attrs.radius, point.attrs.radius);
                    }
                }

                j += 1;
            }

            match self {
                MinMaxAggregator::MinMax => {
                    aggregated.push(acc_min);
                    // Don't push the same point twice.
                    if j > 1 {
                        aggregated.push(acc_max);
                    }
                }
                MinMaxAggregator::MinMaxAverage => {
                    // Don't average a single point with itself.
                    if j > 1 {
                        acc_min.value = (acc_min.value + acc_max.value) * 0.5;
                        acc_min.attrs.radius = (acc_min.attrs.radius + acc_max.attrs.radius) * 0.5;
                    }
                    aggregated.push(acc_min);
                }
                MinMaxAggregator::Min => {
                    aggregated.push(acc_min);
                }
                MinMaxAggregator::Max => {
                    aggregated.push(acc_max);
                }
            }

            i += j;
        }

        // Force align the start and end timestamps to prevent jarring visual glitches.
        if let Some(p) = aggregated.first_mut() {
            p.time = min_time;
        }
        if let Some(p) = aggregated.last_mut() {
            p.time = max_time;
        }

        aggregated
    }
}

/// Are two [`PlotPoint`]s safe to aggregate?
fn are_aggregatable(point1: &PlotPoint, point2: &PlotPoint, window_size: usize) -> bool {
    let PlotPoint {
        time,
        value: _,
        attrs,
    } = point1;
    let PlotPointAttrs {
        label,
        color,
        radius: _,
        scattered,
    } = attrs;

    // We cannot aggregate two points that don't live in the same aggregation window to start with.
    // This is very common with e.g. sparse datasets.
    time.abs_diff(point2.time) <= window_size as u64
        && *label == point2.attrs.label
        && *color == point2.attrs.color
        && *scattered == point2.attrs.scattered
}
