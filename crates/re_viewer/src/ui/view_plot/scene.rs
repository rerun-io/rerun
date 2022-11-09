use crate::{
    ui::{view_2d::Legends, SceneQuery},
    ViewerContext,
};
use ahash::HashMap;
use re_data_store::{
    query::visit_type_data_2, FieldName, ObjPath, ObjectTreeProperties, TimeQuery,
};
use re_log_types::{IndexHash, MsgId, ObjectType};

// ---

#[derive(Clone, Debug)]
pub struct Scalar {
    pub time: i64,
    pub value: f64,
}

/// A plot scene, with everything needed to render it.
#[derive(Default, Debug)]
pub struct ScenePlot {
    pub legends: Legends,
    pub plots: HashMap<ObjPath, Vec<Scalar>>,
}

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
            let mut batch = Vec::new();
            visit_type_data_2(
                obj_store,
                &FieldName::from("scalar"),
                &TimeQuery::EVERYTHING, // always sticky!
                ("_visible", "legend"),
                |_instance_index: Option<&IndexHash>,
                 time: i64,
                 _msg_id: &MsgId,
                 value: &f64,
                 visible: Option<&bool>,
                 legend: Option<&ObjPath>| {
                    if *visible.unwrap_or(&true) {
                        batch.push(Scalar {
                            time,
                            value: *value,
                        });
                    }
                },
            );
            batch.sort_by_key(|s| s.time);

            self.plots
                .entry(obj_path.clone())
                .or_default()
                .extend(batch);
        }
    }
}

impl ScenePlot {
    pub fn is_empty(&self) -> bool {
        let Self { legends: _, plots } = self;

        plots.is_empty()
    }
}
