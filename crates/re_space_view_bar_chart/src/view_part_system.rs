use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_log_types::{TimeInt, Timeline};
use re_types::{archetypes::Tensor, datatypes::TensorData, Archetype, ComponentNameSet};
use re_viewer_context::{
    default_heuristic_filter, NamedViewSystem, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
};

/// A bar chart system, with everything needed to render it.
#[derive(Default)]
pub struct BarChartViewPartSystem {
    pub charts: BTreeMap<EntityPath, TensorData>,
}

impl NamedViewSystem for BarChartViewPartSystem {
    fn name() -> re_viewer_context::ViewSystemName {
        "BarChartView".into()
    }
}

impl ViewPartSystem for BarChartViewPartSystem {
    fn required_components(&self) -> ComponentNameSet {
        // TODO(#3327): make barchart an actual archetype
        Tensor::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Tensor::indicator_component()).collect()
    }

    fn heuristic_filter(
        &self,
        store: &re_arrow_store::DataStore,
        ent_path: &EntityPath,
        entity_components: &ComponentNameSet,
    ) -> bool {
        if !default_heuristic_filter(entity_components, &self.indicator_components()) {
            return false;
        }

        if let Some(tensor) = store.query_latest_component::<re_types::components::TensorData>(
            ent_path,
            &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        ) {
            tensor.is_vector()
        } else {
            false
        }
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let store = &ctx.store_db.entity_db.data_store;

        for (ent_path, _props) in query.iter_entities_for_system(Self::name()) {
            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let tensor =
                store.query_latest_component::<re_types::components::TensorData>(ent_path, &query);

            if let Some(tensor) = tensor {
                if tensor.is_vector() {
                    self.charts.insert(ent_path.clone(), tensor.value.0.clone());
                    // shallow clones
                }
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
