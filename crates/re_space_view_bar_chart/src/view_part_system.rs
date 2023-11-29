use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_types::{
    archetypes::{BarChart, Tensor},
    components::Color,
    datatypes::TensorData,
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    default_heuristic_filter, HeuristicFilterContext, NamedViewSystem,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
};

/// A bar chart system, with everything needed to render it.
#[derive(Default)]
pub struct BarChartViewPartSystem {
    pub charts: BTreeMap<EntityPath, (TensorData, Option<Color>)>,
}

impl NamedViewSystem for BarChartViewPartSystem {
    const NAME: &'static str = "BarChartView";
}

impl ViewPartSystem for BarChartViewPartSystem {
    fn required_components(&self) -> ComponentNameSet {
        BarChart::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        // TODO(#3342): For now, we relax the indicator component heuristics on bar charts so that
        // logging a 1D tensor also results in a bar chart view, rather than a broken viewer (see #3709).
        // Ideally though, this should be implemented using an heuristic fallback mechanism.
        [BarChart::indicator().name(), Tensor::indicator().name()]
            .into_iter()
            .collect()
    }

    fn heuristic_filter(
        &self,
        store: &re_arrow_store::DataStore,
        ent_path: &EntityPath,
        _ctx: HeuristicFilterContext,
        query: &LatestAtQuery,
        entity_components: &ComponentNameSet,
    ) -> bool {
        if !default_heuristic_filter(entity_components, &self.indicator_components()) {
            return false;
        }

        // NOTE: We want to make sure we query at the right time, otherwise we might take into
        // account a `Clear()` that actually only applies into the future, and then
        // `is_vector` will righfully fail because of the empty tensor.
        if let Some(tensor) =
            store.query_latest_component::<re_types::components::TensorData>(ent_path, query)
        {
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

        let store = ctx.store_db.store();

        for data_result in query.iter_visible_data_results(Self::name()) {
            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let tensor = store.query_latest_component::<re_types::components::TensorData>(
                &data_result.entity_path,
                &query,
            );

            let color = store.query_latest_component::<re_types::components::Color>(
                &data_result.entity_path,
                &query,
            );

            if let Some(tensor) = tensor {
                if tensor.is_vector() {
                    self.charts.insert(
                        data_result.entity_path.clone(),
                        (tensor.value.0.clone(), color.map(|c| c.value)),
                    );
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
