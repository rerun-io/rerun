use std::collections::BTreeMap;

use re_data_store::LatestAtQuery;
use re_entity_db::EntityPath;
use re_space_view::diff_component_filter;
use re_types::{
    archetypes::{BarChart, Tensor},
    components::Color,
    datatypes::TensorData,
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery,
    ViewerContext, VisualizerAdditionalApplicabilityFilter, VisualizerSystem,
};

/// A bar chart system, with everything needed to render it.
#[derive(Default)]
pub struct BarChartVisualizerSystem {
    pub charts: BTreeMap<EntityPath, (TensorData, Option<Color>)>,
}

impl IdentifiedViewSystem for BarChartVisualizerSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "BarChartView".into()
    }
}

struct BarChartVisualizerEntityFilter;

impl VisualizerAdditionalApplicabilityFilter for BarChartVisualizerEntityFilter {
    fn update_applicability(&mut self, event: &re_data_store::StoreEvent) -> bool {
        diff_component_filter(event, |tensor: &re_types::components::TensorData| {
            tensor.is_vector()
        })
    }
}

impl VisualizerSystem for BarChartVisualizerSystem {
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

    fn applicability_filter(&self) -> Option<Box<dyn VisualizerAdditionalApplicabilityFilter>> {
        Some(Box::new(BarChartVisualizerEntityFilter))
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        _view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let store = ctx.entity_db.store();

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            let tensor = store
                .query_latest_component::<re_types::components::TensorData>(
                    &data_result.entity_path,
                    &query,
                )
                .map(|(_, data)| data);

            let color = store
                .query_latest_component::<re_types::components::Color>(
                    &data_result.entity_path,
                    &query,
                )
                .map(|(_, data)| data);

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
