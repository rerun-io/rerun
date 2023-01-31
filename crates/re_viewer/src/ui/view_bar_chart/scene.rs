use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_log::warn_once;
use re_log_types::component_types::{self, Instance, Tensor, TensorTrait as _};
use re_query::query_entity_with_primary;

use crate::{misc::ViewerContext, ui::scene::SceneQuery};

/// A bar chart scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneBarChart {
    pub charts: BTreeMap<(EntityPath, Instance), Tensor>,
}

impl SceneBarChart {
    pub(crate) fn load(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.load_tensors(ctx, query);
    }

    fn load_tensors(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let store = &ctx.log_db.entity_db.arrow_store;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let ent_view =
                query_entity_with_primary::<component_types::Tensor>(store, &query, ent_path, &[]);
            let Ok(ent_view) = ent_view else {
                warn_once!("bar chart query failed for {:?}", ent_path);
                continue;
            };
            let Ok(instances) = ent_view.iter_instances() else {
                warn_once!("bar chart query failed for {:?}", ent_path);
                continue;
            };
            let Ok(tensors) = ent_view.iter_primary() else {
                warn_once!("bar chart query failed for {:?}", ent_path);
                continue;
            };

            for (instance, tensor) in instances.zip(tensors) {
                let tensor = tensor.unwrap(); // primary
                if tensor.is_vector() {
                    self.charts.insert(
                        (ent_path.clone(), instance),
                        tensor.clone(), /* shallow */
                    );
                }
            }
        }
    }
}
