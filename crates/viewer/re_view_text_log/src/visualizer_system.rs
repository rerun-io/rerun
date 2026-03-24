use re_sdk_types::archetypes::TextLog;
use re_viewer_context::{
    IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

/// Marker visualizer that keeps text-log entities discoverable for the view system.
#[derive(Default)]
pub struct TextLogSystem;

impl IdentifiedViewSystem for TextLogSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TextLog".into()
    }
}

impl VisualizerSystem for TextLogSystem {
    /// Declares that this visualizer is driven by the `TextLog` archetype.
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<TextLog>()
    }

    /// Leaves table data loading to the cached view path while keeping instruction wiring intact.
    fn execute(
        &mut self,
        _ctx: &ViewContext<'_>,
        _view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        Ok(VisualizerExecutionOutput::default())
    }
}
