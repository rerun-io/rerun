use std::collections::BTreeMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityPath;
use re_types::{
    archetypes::BarChart,
    components::{self},
    datatypes,
};
use re_view::DataResultQuery as _;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizerQueryInfo,
    VisualizerSystem, auto_color_for_entity_path,
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

impl VisualizerSystem for BarChartVisualizerSystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<BarChart>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let timeline_query = LatestAtQuery::new(view_query.timeline, view_query.latest_at);

        for data_result in view_query.iter_visible_data_results(Self::identifier()) {
            let results = data_result
                .latest_at_with_blueprint_resolved_data::<BarChart>(ctx, &timeline_query);

            let Some(tensor) =
                results.get_required_mono::<components::TensorData>(&BarChart::descriptor_values())
            else {
                continue;
            };

            if tensor.is_vector() {
                let color = results.get_mono_with_fallback(&BarChart::descriptor_color(), self);
                self.charts
                    .insert(data_result.entity_path.clone(), (tensor.0.clone(), color));
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<components::Color> for BarChartVisualizerSystem {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> components::Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

re_viewer_context::impl_component_fallback_provider!(BarChartVisualizerSystem => [components::Color]);
