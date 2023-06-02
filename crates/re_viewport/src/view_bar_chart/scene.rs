use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_components::Tensor;
use re_data_store::EntityPath;
use re_viewer_context::{SceneQuery, ViewerContext};

/// A bar chart scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneBarChart {
    pub charts: BTreeMap<EntityPath, Tensor>,
}

impl SceneBarChart {
    pub(crate) fn load(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        re_tracing::profile_function!();

        self.load_tensors(ctx, query);
    }

    fn load_tensors(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        re_tracing::profile_function!();

        let store = &ctx.store_db.entity_db.data_store;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let tensor = store.query_latest_component::<Tensor>(ent_path, &query);

            if let Some(tensor) = tensor {
                if tensor.is_vector() {
                    self.charts.insert(ent_path.clone(), tensor.clone()); // shallow clones
                }
            }
        }
    }
}
