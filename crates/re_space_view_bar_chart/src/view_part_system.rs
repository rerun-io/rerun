use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_components::Tensor;
use re_data_store::EntityPath;
use re_log_types::Component as _;
use re_viewer_context::{
    ArchetypeDefinition, SpaceViewClass, SpaceViewHighlights, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
};

use crate::BarChartSpaceView;

/// A bar chart system, with everything needed to render it.
#[derive(Default)]
pub struct BarChartViewPartSystem {
    pub charts: BTreeMap<EntityPath, Tensor>,
}

impl ViewPartSystem for BarChartViewPartSystem {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Tensor::name()]
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
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

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
