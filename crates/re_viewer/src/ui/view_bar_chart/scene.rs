use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::{query_latest_single, EntityPath};
use re_log_types::component_types::{self, InstanceKey, Tensor};
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

        let entity_db = &ctx.log_db.entity_db;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let tensor =
                query_latest_single::<component_types::Tensor>(entity_db, ent_path, &query);

            if let Some(tensor) = tensor {
                if tensor.is_vector() {
                    self.charts.insert(
                        (ent_path.clone(), InstanceKey(0)),
                        tensor.clone(), /* shallow */
                    );
                }
            }
        }
    }
}
