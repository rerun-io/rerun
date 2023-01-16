use crate::{
    ui::{annotations::AnnotationMap, DefaultColor, SceneQuery},
    ViewerContext,
};
use re_arrow_store::TimeRange;
use re_data_store::{query::visit_type_data_4, FieldName, TimeQuery};
use re_log_types::{
    field_types::{self, Instance},
    msg_bundle::Component,
    IndexHash, MsgId, ObjectType,
};
use re_query::{range_entity_with_primary, QueryError};

// ---

#[derive(Clone, Debug)]
pub struct PlotPointAttrs {
    pub label: Option<String>,
    pub color: [u8; 4],
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
    pub color: [u8; 4],
    pub width: f32,
    pub kind: PlotSeriesKind,
    pub points: Vec<(i64, f64)>,
}

/// A scene for a time series plot, with everything needed to render it.
#[derive(Default, Debug)]
pub struct SceneTimeSeries {
    pub annotation_map: AnnotationMap,
    pub lines: Vec<PlotSeries>,
}

impl SceneTimeSeries {
    /// Loads all plot objects into the scene according to the given query.
    pub(crate) fn load_objects(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.annotation_map.load(ctx, query);

        self.load_scalars(ctx, query);

        self.load_scalars_arrow(ctx, query);
    }

    fn load_scalars(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (_obj_type, obj_path, _time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Scalar])
        {
            let mut points = Vec::new();
            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            visit_type_data_4(
                obj_store,
                &FieldName::from("scalar"),
                &TimeQuery::EVERYTHING,
                ("label", "color", "radius", "scattered"),
                |_instance_index: Option<&IndexHash>,
                 time: i64,
                 _msg_id: &MsgId,
                 value: &f64,
                 label: Option<&String>,
                 color: Option<&[u8; 4]>,
                 radius: Option<&f32>,
                 scattered: Option<&bool>| {
                    // TODO(andreas): Support object path
                    let annotation_info = annotations.class_description(None).annotation_info();
                    let color = annotation_info.color(color, default_color);
                    let label = annotation_info.label(label);

                    points.push(PlotPoint {
                        time,
                        value: *value,
                        attrs: PlotPointAttrs {
                            label,
                            color,
                            radius: radius.copied().unwrap_or(1.0),
                            scattered: *scattered.unwrap_or(&false),
                        },
                    });
                },
            );
            points.sort_by_key(|s| s.time);

            if points.is_empty() {
                continue;
            }

            // If all points within a line share the label (and it isn't `None`), then we use it
            // as the whole line label for the plot legend.
            // Otherwise, we just use the object path as-is.
            let same_label = |points: &[PlotPoint]| {
                let label = points[0].attrs.label.as_ref();
                (label.is_some() && points.iter().all(|p| p.attrs.label.as_ref() == label))
                    .then(|| label.cloned().unwrap())
            };
            let line_label = same_label(&points).unwrap_or_else(|| obj_path.to_string());

            self.add_line_segments(&line_label, points);
        }
    }

    fn load_scalars_arrow(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let store = &ctx.log_db.obj_db.arrow_store;

        for obj_path in query.obj_paths {
            let ent_path = obj_path;

            let mut points = Vec::new();
            let annotations = self.annotation_map.find(ent_path);
            let default_color = DefaultColor::ObjPath(ent_path);

            let query = re_arrow_store::RangeQuery::new(
                query.timeline,
                TimeRange::new(i64::MIN.into(), i64::MAX.into()),
            );

            let components = [
                Instance::name(),
                field_types::Scalar::name(),
                field_types::ScalarPlotProps::name(),
                field_types::ColorRGBA::name(),
                field_types::Radius::name(),
                field_types::Label::name(),
            ];
            let ent_views = range_entity_with_primary::<field_types::Scalar, 6>(
                store, &query, ent_path, components,
            );

            for (time, ent_view) in ent_views {
                match ent_view.visit5(
                    |_instance,
                     scalar: field_types::Scalar,
                     props: Option<field_types::ScalarPlotProps>,
                     color: Option<field_types::ColorRGBA>,
                     radius: Option<field_types::Radius>,
                     label: Option<field_types::Label>| {
                        // TODO(andreas): Support object path
                        let annotation_info = annotations.class_description(None).annotation_info();
                        let color = annotation_info
                            .color(color.map(|c| c.to_array()).as_ref(), default_color);
                        let label = annotation_info.label(label.map(|l| l.into()).as_ref());

                        points.push(PlotPoint {
                            time: time.unwrap().as_i64(), // scalars cannot be timeless
                            value: scalar.into(),
                            attrs: PlotPointAttrs {
                                label,
                                color,
                                radius: radius.map_or(1.0, |r| r.into()),
                                scattered: props.map_or(false, |props| props.scattered),
                            },
                        });
                    },
                ) {
                    Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                    Err(err) => {
                        re_log::error_once!("Unexpected error querying '{ent_path:?}': {err:?}");
                    }
                }
            }

            points.sort_by_key(|s| s.time);

            if points.is_empty() {
                continue;
            }

            // If all points within a line share the label (and it isn't `None`), then we use it
            // as the whole line label for the plot legend.
            // Otherwise, we just use the object path as-is.
            let same_label = |points: &[PlotPoint]| {
                let label = points[0].attrs.label.as_ref();
                (label.is_some() && points.iter().all(|p| p.attrs.label.as_ref() == label))
                    .then(|| label.cloned().unwrap())
            };
            let line_label = same_label(&points).unwrap_or_else(|| obj_path.to_string());

            self.add_line_segments(&line_label, points);
        }
    }

    // We have a bunch of raw points, and now we need to group them into actual line
    // segments.
    // A line segment is a continuous run of points with identical attributes: each time
    // we notice a change in attributes, we need a new line segment.
    fn add_line_segments(&mut self, line_label: &str, points: Vec<PlotPoint>) {
        let nb_points = points.len();
        let mut attrs = points[0].attrs.clone();
        let mut line: PlotSeries = PlotSeries {
            label: line_label.to_owned(),
            color: attrs.color,
            width: attrs.radius,
            kind: if attrs.scattered {
                PlotSeriesKind::Scatter
            } else {
                PlotSeriesKind::Continuous
            },
            points: Vec::with_capacity(nb_points),
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
                        width: attrs.radius,
                        kind,
                        points: Vec::with_capacity(nb_points - i),
                    },
                );
                let prev_point = *prev_line.points.last().unwrap();
                self.lines.push(prev_line);

                // If the previous point was continous and the current point is continuous
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
