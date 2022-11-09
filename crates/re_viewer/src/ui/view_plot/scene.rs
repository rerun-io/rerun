use crate::{ui::SceneQuery, ViewerContext};
use ahash::HashMap;
use re_data_store::{
    query::visit_type_data_4, FieldName, ObjPath, ObjectTreeProperties, TimeQuery,
};
use re_log_types::{IndexHash, MsgId, ObjectType};

// ---

// TODO:
// - do everything _per point_
// - if no legend: derive what we can for the whole line
// - if legend: well, use that
//
// TODO:
// - a plot is a space
// - a line is an object path within that space
// - a point is a scalar logged to that object path
//
// TODO:
// - point-level props
//   - label, color, radius
//   -> read as-is from the Scalar object
//   -> missing ones are defaulted:
//      - label: None
//      - color: derived from obj_path hash
//      - radius: 1.0
// - line-level props
//   - label, color, width, kind (e.g. enum{scatter, line})
//   -> read as-is from the legend object associated with that obj_path (future PR?)
//   -> otherwise the missing ones are defaulted:
//      - label: obj_path
//      - color:
//          - if all points share same color, use that
//          - otherwise, derived from obj_path hash
//      - width:
//          - if all points share same radius, use that
//          - otherwise, 1.0
//      - kind:
//          - if all points share same color, default to line, scatter otherwise
//   -> in the future, those should be modifiable at run-time from the blueprint UI
// - plot-level props
//   - label
//   - sticky
//   -> as-is from the legend object associated with that space (annotation context??)
//   -> otherwise the missing ones are defaulted:
//      - label: space name
//   -> in the future, those should be modifiable at run-time from the blueprint UI

#[derive(Clone, Debug)]
pub struct PlotPoint {
    pub time: i64,
    pub value: f64,
    // TODO: egui plots don't support attributes below the line-level at the moment
    pub label: Option<String>, // TODO: yeah we need an Arc in the storage layer
    pub color: [u8; 4],        // TODO: make the Color32 PR
    pub radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum PlotLineKind {
    Continuous,
    Scatter,
}

#[derive(Clone, Debug)]
pub struct PlotLine {
    pub label: String,
    pub color: [u8; 4], // TODO: make the Color32 PR
    pub width: f32,
    pub kind: PlotLineKind,
    pub points: Vec<PlotPoint>,
}

/// A plot scene, with everything needed to render it.
#[derive(Default, Debug)]
pub struct ScenePlot {
    pub lines: Vec<PlotLine>,
}

// TODO: document all the logic of how colors get selected and stuff

impl ScenePlot {
    /// Loads all plot objects into the scene according to the given query.
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        obj_tree_props: &ObjectTreeProperties,
        query: &SceneQuery<'_>,
    ) {
        crate::profile_function!();

        self.load_scalars(ctx, obj_tree_props, query);
    }

    fn load_scalars(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        obj_tree_props: &ObjectTreeProperties,
        query: &SceneQuery<'_>,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, obj_store) in
            query.iter_object_stores(ctx.log_db, obj_tree_props, &[ObjectType::Scalar])
        {
            let default_color = {
                let c = ctx.cache.random_color(obj_path);
                [c[0], c[1], c[2], 255]
            };

            let mut points = Vec::new();
            visit_type_data_4(
                obj_store,
                &FieldName::from("scalar"),
                &TimeQuery::EVERYTHING, // always sticky!
                ("_visible", "label", "color", "radius"),
                |_instance_index: Option<&IndexHash>,
                 time: i64,
                 _msg_id: &MsgId,
                 value: &f64,
                 visible: Option<&bool>,
                 label: Option<&String>,
                 color: Option<&[u8; 4]>,
                 radius: Option<&f32>| {
                    if !*visible.unwrap_or(&true) {
                        return;
                    }

                    points.push(PlotPoint {
                        time,
                        label: label.cloned(),
                        color: color.copied().unwrap_or(default_color),
                        radius: radius.copied().unwrap_or(1.0),
                        value: *value,
                    });
                },
            );
            points.sort_by_key(|s| s.time);

            if points.is_empty() {
                continue;
            }

            let label = 'label: {
                let label = points[0].label.as_ref();
                if label.is_some() && points.iter().all(|p| p.label.as_ref() == label) {
                    break 'label label.cloned().unwrap();
                }
                obj_path.to_string()
            };

            let kind = 'kind: {
                let color = points[0].color;
                if points.iter().all(|p| p.color == color) {
                    break 'kind PlotLineKind::Continuous;
                }
                PlotLineKind::Scatter
            };

            let width = 'width: {
                let radius = points[0].radius;
                if points.iter().all(|p| p.radius == radius) {
                    break 'width radius;
                }
                1.0
            };

            // TODO: one could argue this should be done in the ui file, since this is done
            // only to work around a limitation of egui plots... but then again it's easier
            // to do here sooo...

            let mut color = points[0].color;
            let mut nb_points = points.len();

            let mut line: Option<PlotLine> = Some(PlotLine {
                label: label.clone(),
                color,
                width,
                kind,
                points: Vec::with_capacity(nb_points),
            });

            for p in points.into_iter() {
                if p.color == color {
                    line.as_mut().unwrap().points.push(p);
                } else {
                    let taken = line.take().unwrap();

                    nb_points -= taken.points.len();
                    self.lines.push(taken);

                    color = p.color;
                    line = Some(PlotLine {
                        label: label.clone(),
                        color,
                        width,
                        kind,
                        points: Vec::with_capacity(nb_points),
                    });
                    line.as_mut().unwrap().points.push(p);
                }
            }

            if !line.as_ref().unwrap().points.is_empty() {
                self.lines.push(line.take().unwrap());
            }
        }
    }
}

impl ScenePlot {
    pub fn is_empty(&self) -> bool {
        let Self { lines: plots } = self;

        plots.is_empty()
    }
}
