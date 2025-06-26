use re_viewer_context::{
    ComponentFallbackProvider, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerQueryInfo, VisualizerSystem,
};

/// An empty system to accept all entities in the view
#[derive(Default)]
pub struct EmptySystem {}

impl IdentifiedViewSystem for EmptySystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Empty".into()
    }
}

impl VisualizerSystem for EmptySystem {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::empty()
    }

    fn execute(
        &mut self,
        _ctx: &ViewContext<'_>,
        _query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        Ok(vec![])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl ComponentFallbackProvider for EmptySystem {
    fn try_provide_fallback(
        &self,
        _ctx: &re_viewer_context::QueryContext<'_>,
        _component: re_types_core::ComponentType,
    ) -> re_viewer_context::ComponentFallbackProviderResult {
        re_viewer_context::ComponentFallbackProviderResult::ComponentNotHandled
    }
}
