use std::collections::BTreeMap;

use re_chunk_store::ChunkStoreEvent;
use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityPath;
use re_space_view::{diff_component_filter, DataResultQuery2 as _};
use re_types::{
    archetypes::BarChart,
    components::{self},
    datatypes,
};
use re_viewer_context::{
    auto_color_for_entity_path, IdentifiedViewSystem, QueryContext, SpaceViewSystemExecutionError,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    VisualizerAdditionalApplicabilityFilter, VisualizerQueryInfo, VisualizerSystem,
};

/// A bar chart system, with everything needed to render it.
#[derive(Default)]
pub struct BarChartVisualizerSystem {
    pub charts: BTreeMap<EntityPath, (datatypes::TensorData, components::Color)>,
}

impl IdentifiedViewSystem for BarChartVisualizerSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "BarChart".into()
    }
}

struct BarChartVisualizerEntityFilter;

impl VisualizerAdditionalApplicabilityFilter for BarChartVisualizerEntityFilter {
    #[inline]
    fn update_applicability(&mut self, event: &ChunkStoreEvent) -> bool {
        diff_component_filter(event, |tensor: &re_types::components::TensorData| {
            tensor.is_vector()
        })
    }
}

impl VisualizerSystem for BarChartVisualizerSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<BarChart>()
    }

    fn applicability_filter(&self) -> Option<Box<dyn VisualizerAdditionalApplicabilityFilter>> {
        Some(Box::new(BarChartVisualizerEntityFilter))
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for data_result in view_query.iter_visible_data_results(ctx, Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<BarChart>(ctx, &timeline_query);

            let Some(tensor) = results.get_required_mono::<components::TensorData>() else {
                continue;
            };

            if tensor.is_vector() {
                let color = results.get_mono_with_fallback();
                self.charts
                    .insert(data_result.entity_path.clone(), (tensor.0.clone(), color));
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<components::Color> for BarChartVisualizerSystem {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> components::Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

re_viewer_context::impl_component_fallback_provider!(BarChartVisualizerSystem => [components::Color]);
