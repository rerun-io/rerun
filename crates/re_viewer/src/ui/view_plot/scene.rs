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
// - plot-level props
//   - label
//   -> as-is from the legend object associated with that space (annotation context??)
//   -> otherwise the missing ones are defaulted:
//      - label: space name
//
// TODO:
// - a plot is a space
// - a line is an object path within that space
// - a point is a scalar logged to that object path

#[derive(Clone, Debug)]
pub struct PlotPoint {
    pub time: i64,
    pub value: f64,
    pub radius: f32,
    pub label: Option<String>, // TODO: yeah we need an Arc in the storage layer
}

#[derive(Clone, Debug)]
pub struct PlotLine {
    // TODO: how do we derive the line label then?
    pub label: String, // TODO: yeah we need an Arc in the storage layer
    // TODO: how do we derive the line color then?!
    pub color: [u8; 4], // TODO: make the Color32 PR
    // TODO: how do we derive the line
    pub width: f32,
    pub points: Vec<PlotPoint>,
}

/// A plot scene, with everything needed to render it.
#[derive(Default, Debug)]
pub struct ScenePlot {
    pub plots: HashMap<ObjPath, Vec<PlotLine>>,
}

// TODO: document all the logic of how colors get selected and stuff

impl ScenePlot {
    /// Loads all plot objects into the scene according to the given query.
    pub(crate) fn load_objects(
        &mut self,
        ctx: &ViewerContext<'_>,
        obj_tree_props: &ObjectTreeProperties,
        query: &SceneQuery<'_>,
    ) {
        crate::profile_function!();

        self.load_scalars(ctx, obj_tree_props, query);
    }

    fn load_scalars(
        &mut self,
        ctx: &ViewerContext<'_>,
        obj_tree_props: &ObjectTreeProperties,
        query: &SceneQuery<'_>,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, obj_store) in
            query.iter_object_stores(ctx.log_db, obj_tree_props, &[ObjectType::Scalar])
        {
            let mut scalars = Vec::new();

            let mut cur_color = {
                let c = ctx.cache.random_color(obj_path);
                [c[0], c[1], c[2], 255]
            };

            visit_type_data_4(
                obj_store,
                &FieldName::from("scalar"),
                &TimeQuery::EVERYTHING, // always sticky!
                ("_visible", "laebel", "color", "radius"),
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

                    let color = color.copied().unwrap_or(cur_color);

                    scalars.push(Scalar {
                        time,
                        label: label.cloned(),
                        color,
                        radius: radius.copied().unwrap_or(1.0),
                        value: *value,
                    });
                },
            );

            if let Some(lines) = scalars_to_lines(scalars) {
                self.plots
                    .entry(obj_path.clone())
                    .or_default()
                    .extend(lines);
            }
        }
    }
}

impl ScenePlot {
    pub fn is_empty(&self) -> bool {
        let Self { plots } = self;

        plots.is_empty()
    }
}

// ---

struct Scalar {
    time: i64,
    label: Option<String>, // TODO: yeah we need an Arc in the storage layer
    color: [u8; 4],        // TODO: make the Color32 PR
    radius: f32,
    value: f64,
}

fn scalars_to_lines(
    scalars: impl IntoIterator<Item = Scalar>,
) -> Option<impl Iterator<Item = PlotLine>> {
    let mut scalars = scalars.into_iter().collect::<Vec<_>>();
    scalars.sort_by_key(|s| s.time);

    if scalars.is_empty() {
        return None;
    }

    let mut line = PlotLine {
        label: "MyLineLabel".to_owned(),
        color: todo!(),
        points: scalars.into_iter().map(|s| PlotPoint {
            time: s.time,
            value: s.value,
            radius: s.radius,
            label: todo!(),
        }),
    };

    Some(std::iter::once(line))
}
