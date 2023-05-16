use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_error::ResultExt as _;
use re_log_types::component_types::{self, InstanceKey, Tensor};
use re_query::query_entity_with_primary;
use re_viewer_context::{SceneQuery, ViewerContext};

/// A bar chart scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneBarChart {
    pub charts: BTreeMap<(EntityPath, InstanceKey), Tensor>,
}

impl SceneBarChart {
    pub(crate) fn load(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_tensors(ctx, query);
    }

    fn load_tensors(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let store = &ctx.log_db.entity_db.data_store;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let ent_view =
                query_entity_with_primary::<component_types::Tensor>(store, &query, ent_path, &[]);
            let Some(ent_view) = ent_view.warn_on_err_once(format!("Bar chart query failed for {ent_path:?}")) else {
                continue;
            };
            let Some(instance_keys) = ent_view.iter_instance_keys().warn_on_err_once(format!("Bar chart query failed for {ent_path:?}")) else {
                continue;
            };
            let Some(tensors) = ent_view.iter_primary().warn_on_err_once(format!("Bar chart query failed for {ent_path:?}")) else {
                continue;
            };

            for (instance_key, tensor) in instance_keys.zip(tensors) {
                let tensor = tensor.unwrap(); // primary
                if tensor.is_vector() {
                    self.charts.insert(
                        (ent_path.clone(), instance_key),
                        tensor.clone(), /* shallow */
                    );
                }
            }
        }
    }
}
