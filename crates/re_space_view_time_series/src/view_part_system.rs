use itertools::Itertools;
use re_arrow_store::TimeRange;
use re_query::QueryError;
use re_types::{
    archetypes::TimeSeriesScalar,
    components::{Color, Radius, Scalar, ScalarScattering, Text},
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    AnnotationMap, DefaultColor, NamedViewSystem, SpaceViewSystemExecutionError, ViewPartSystem,
    ViewQuery, ViewerContext,
};

use crate::space_view_class::TimeSeriesSpaceViewFeedback;

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

// TODO: we need custom view cache for this stuff, on top of generic query cache
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
}

impl NamedViewSystem for TimeSeriesSystem {
    fn name() -> re_viewer_context::ViewSystemName {
        "TimeSeries".into()
    }
}

impl ViewPartSystem for TimeSeriesSystem {
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
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _context: &re_viewer_context::ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        self.annotation_map.load(
            ctx,
            &query.latest_at_query(),
            query
                .iter_visible_data_results(Self::name())
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

        let store = ctx.store_db.store();

        let ui_feedback = TimeSeriesSpaceViewFeedback::remove(&query.space_view_id);
        let x_tick_size = ui_feedback
            .map(|feedback| {
                feedback.plot_bounds.width() / feedback.plot_canvas_size.x.max(0.001) as f64
            })
            .unwrap_or(0.0);

        for data_result in query.iter_visible_data_results(Self::name()) {
            let annotations = self.annotation_map.find(&data_result.entity_path);
            let annotation_info = annotations
                .resolved_class_description(None)
                .annotation_info();
            let default_color = DefaultColor::EntityPath(&data_result.entity_path);

            let visible_history = match query.timeline.typ() {
                re_log_types::TimeType::Time => {
                    data_result.resolved_properties.visible_history.nanos
                }
                re_log_types::TimeType::Sequence => {
                    data_result.resolved_properties.visible_history.sequences
                }
            };

            let (from, to) = if data_result.resolved_properties.visible_history.enabled {
                (
                    visible_history.from(query.latest_at),
                    visible_history.to(query.latest_at),
                )
            } else {
                (i64::MIN.into(), i64::MAX.into())
            };

            let query = re_arrow_store::RangeQuery::new(query.timeline, TimeRange::new(from, to));

            re_query_cache::query_cached_archetype_r1o4::<
                { TimeSeriesScalar::NUM_COMPONENTS },
                TimeSeriesScalar,
                Scalar,
                ScalarScattering,
                Color,
                Radius,
                Text,
                _,
            >(
                store,
                &query.clone().into(),
                &data_result.entity_path,
                |it| {
                    re_tracing::profile_function!();

                    let mut points = Vec::new();

                    // if x_tick_size.floor() > 1.0 {
                    //     re_tracing::profile_scope!("build (decimated)");
                    //
                    //     for ((time, _row_id), _, scalars, scatterings, colors, radii, labels) in
                    //         it.step_by(x_tick_size.floor() as usize)
                    //     {
                    //         for (scalar, scattered, color, radius, label) in
                    //             itertools::izip!(scalars, scatterings, colors, radii, labels)
                    //         {
                    //             let color = annotation_info
                    //                 .color(color.map(|c| c.to_array()), default_color);
                    //             let label =
                    //                 annotation_info.label(label.as_ref().map(|l| l.as_str()));
                    //
                    //             const DEFAULT_RADIUS: f32 = 0.75;
                    //
                    //             points.push(PlotPoint {
                    //                 time: time.as_i64(),
                    //                 value: scalar.0,
                    //                 attrs: PlotPointAttrs {
                    //                     label,
                    //                     color,
                    //                     radius: radius.map_or(DEFAULT_RADIUS, |r| r.0),
                    //                     scattered: scattered.map_or(false, |s| s.0),
                    //                 },
                    //             });
                    //         }
                    //     }
                    // } else
                    if x_tick_size > 1.0 {
                        // eprintln!("aggregating! {x_tick_size}");
                        re_tracing::profile_scope!("build (aggregated)");

                        let windowsz = x_tick_size.ceil() as usize;

                        // TODO: decimal values means including extra data, but not stealing it
                        // from your neighbor!!

                        loop {
                            let mut acc: Vec<Option<PlotPoint>> = vec![None; windowsz];

                            for i in 0..windowsz {
                                let Some((
                                    (time, _row_id),
                                    _,
                                    scalars,
                                    scatterings,
                                    colors,
                                    radii,
                                    labels,
                                )) = it.next()
                                else {
                                    break;
                                };

                                for (scalar, scattered, color, radius, label) in
                                    itertools::izip!(scalars, scatterings, colors, radii, labels)
                                {
                                    let color = annotation_info
                                        .color(color.map(|c| c.to_array()), default_color);
                                    let label =
                                        annotation_info.label(label.as_ref().map(|l| l.as_str()));

                                    const DEFAULT_RADIUS: f32 = 0.75;

                                    acc[i] = Some(PlotPoint {
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
                            }

                            let aggregated = acc.drain(..).flatten().reduce(|mut acc, point| {
                                // TODO: destruct for fwd compat
                                acc.time = i64::max(acc.time, point.time);
                                acc.value = f64::max(acc.value, point.value);
                                acc.attrs.label = point.attrs.label;
                                acc.attrs.color = point.attrs.color;
                                acc.attrs.radius = f32::max(acc.attrs.radius, point.attrs.radius);
                                acc.attrs.scattered =
                                    bool::max(acc.attrs.scattered, point.attrs.scattered);
                                acc
                            });

                            if aggregated.is_none() {
                                break;
                            }

                            points.extend(aggregated);
                        }
                    } else {
                        re_tracing::profile_scope!("build");
                        for ((time, _row_id), _, scalars, scatterings, colors, radii, labels) in it
                        {
                            for (scalar, scattered, color, radius, label) in
                                itertools::izip!(scalars, scatterings, colors, radii, labels)
                            {
                                let color = annotation_info
                                    .color(color.map(|c| c.to_array()), default_color);
                                let label =
                                    annotation_info.label(label.as_ref().map(|l| l.as_str()));

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
                        }
                    }

                    // TODO: seriously i dont get it, what's with the sort? doesnt seem to have
                    // much impact though (because it's already sorted i assume).
                    // TODO: cache should already be sorted at this point.
                    // points.sort_by_key(|s| s.time);
                    assert!(!points.windows(2).any(|p| p[0].time > p[1].time));

                    if points.is_empty() {
                        return;
                    }

                    let points = &points;

                    // TODO: can be cached too
                    // If all points within a line share the label (and it isn't `None`), then we use it
                    // as the whole line label for the plot legend.
                    // Otherwise, we just use the entity path as-is.
                    let same_label = |points: &[PlotPoint]| -> Option<String> {
                        let label = points[0].attrs.label.as_ref()?;
                        (points.iter().all(|p| p.attrs.label.as_ref() == Some(label)))
                            .then(|| label.clone())
                    };
                    let line_label =
                        same_label(points).unwrap_or_else(|| data_result.entity_path.to_string());

                    self.add_line_segments(&line_label, points);

                    let min_time = store
                        .entity_min_time(&query.timeline, &data_result.entity_path)
                        .map_or(points.first().map_or(0, |p| p.time), |time| time.as_i64());

                    self.min_time = Some(self.min_time.map_or(min_time, |time| time.min(min_time)));
                },
            );
        }

        Ok(())
    }

    // We have a bunch of raw points, and now we need to group them into actual line
    // segments.
    // A line segment is a continuous run of points with identical attributes: each time
    // we notice a change in attributes, we need a new line segment.
    //
    // TODO: sure it's slow, but that's because it needs:
    // - to be done on the GPU
    // - to compute level of details
    // - to not split lines for imperceptible changes
    // - etc
    //
    // and then does it really make sense to even cache it? who knows
    #[inline(never)] // Better callstacks on crashes
    fn add_line_segments(&mut self, line_label: &str, points: &[PlotPoint]) {
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

        for (i, p) in points.iter().enumerate() {
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
