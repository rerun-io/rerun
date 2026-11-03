use super::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    ViewSystemIdentifier, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

/// Makes all entities eligible for a view without producing visualization output.
///
/// Views that query the store and render their UI directly, such as Dataframe and Raw Data,
/// still need a registered visualizer for entity eligibility. Use this no-op implementation
/// when the view does not consume visualizer output.
#[derive(Default)]
pub struct NoopVisualizer;

impl IdentifiedViewSystem for NoopVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        re_string_interner::intern_static_nonempty!(ViewSystemIdentifier, "Empty")
    }
}

impl VisualizerSystem for NoopVisualizer {
    fn visualizer_query_info(&self, _app_options: &crate::AppOptions) -> VisualizerQueryInfo {
        VisualizerQueryInfo::empty()
    }

    fn execute(
        &self,
        _ctx: &ViewContext<'_>,
        _query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        Ok(VisualizerExecutionOutput::default())
    }
}
