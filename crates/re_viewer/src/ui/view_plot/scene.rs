use crate::{ui::SceneQuery, ViewerContext};
use re_data_store::{query::visit_type_data_5, FieldName, TimeQuery};
use re_log_types::{IndexHash, MsgId, ObjectType};

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

/// A plot scene, with everything needed to render it.
#[derive(Default, Debug)]
pub struct ScenePlot {
    pub lines: Vec<PlotSeries>,
}

impl ScenePlot {
    /// Loads all plot objects into the scene according to the given query.
    pub(crate) fn load_objects(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_scalars(ctx, query);
    }

    fn load_scalars(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (_obj_type, obj_path, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Scalar])
        {
            let default_color = {
                let c = ctx.cache.random_color(obj_path);
                [c[0], c[1], c[2], 255]
            };

            let mut points = Vec::new();
            visit_type_data_5(
                obj_store,
                &FieldName::from("scalar"),
                &TimeQuery::EVERYTHING,
                ("_visible", "label", "color", "radius", "scattered"),
                |_instance_index: Option<&IndexHash>,
                 time: i64,
                 _msg_id: &MsgId,
                 value: &f64,
                 visible: Option<&bool>,
                 label: Option<&String>,
                 color: Option<&[u8; 4]>,
                 radius: Option<&f32>,
                 scattered: Option<&bool>| {
                    let visible = *visible.unwrap_or(&true);
                    if !visible {
                        return;
                    }

                    points.push(PlotPoint {
                        time,
                        value: *value,
                        attrs: PlotPointAttrs {
                            label: label.cloned(),
                            color: color.copied().unwrap_or(default_color),
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

    // We have a bunch of raw points, and now we need to group them into actual line
    // segments.
    // A line segment is a continuous run of points with identical attributes: each time
    // we notice a change in attributes, we need a new line segment.
    fn add_line_segments(&mut self, line_label: &str, points: Vec<PlotPoint>) {
        let nb_points = points.len();
        let mut attrs = points[0].attrs.clone();
        let mut line: Option<PlotSeries> = Some(PlotSeries {
            label: line_label.to_owned(),
            color: attrs.color,
            width: attrs.radius,
            kind: if attrs.scattered {
                PlotSeriesKind::Scatter
            } else {
                PlotSeriesKind::Continuous
            },
            points: Vec::with_capacity(nb_points),
        });

        for (i, p) in points.into_iter().enumerate() {
            if p.attrs == attrs {
                // Same attributes, just add to the current line segment.

                line.as_mut().unwrap().points.push((p.time, p.value));
            } else {
                // Attributes changed since last point, break up the current run into a
                // line segment, and start the next one.

                let taken = line.take().unwrap();
                self.lines.push(taken);

                attrs = p.attrs.clone();
                let kind = if attrs.scattered {
                    PlotSeriesKind::Scatter
                } else {
                    PlotSeriesKind::Continuous
                };
                line = Some(PlotSeries {
                    label: line_label.to_owned(),
                    color: attrs.color,
                    width: attrs.radius,
                    kind,
                    points: Vec::with_capacity(nb_points - i),
                });

                let prev_line = self.lines.last().unwrap();
                let prev_point = *prev_line.points.last().unwrap();

                // If the previous point was continous and the current point is continuous
                // too, then we want the 2 segments to appear continuous even though they
                // are actually split from a data standpoint.
                let cur_continuous = matches!(kind, PlotSeriesKind::Continuous);
                let prev_continuous = matches!(kind, PlotSeriesKind::Continuous);
                if cur_continuous && prev_continuous {
                    line.as_mut().unwrap().points.push(prev_point);
                }

                // Add the point that triggered the split to the new segment.
                line.as_mut().unwrap().points.push((p.time, p.value));
            }
        }

        if !line.as_ref().unwrap().points.is_empty() {
            self.lines.push(line.take().unwrap());
        }
    }
}

impl ScenePlot {
    pub fn is_empty(&self) -> bool {
        let Self { lines } = self;

        lines.is_empty()
    }
}
