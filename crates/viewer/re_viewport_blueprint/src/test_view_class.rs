//! A minimal view class for testing.

use re_chunk::EntityPath;
use re_sdk_types::ViewClassIdentifier;
use re_viewer_context::{
    IdentifiedViewSystem, SystemExecutionOutput, ViewClass, ViewClassLayoutPriority,
    ViewClassRegistryError, ViewContext, ViewContextCollection, ViewQuery, ViewSpawnHeuristics,
    ViewState, ViewSystemExecutionError, ViewSystemIdentifier, ViewSystemRegistrator,
    ViewerContext, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

/// A minimal visualizer for testing.
#[derive(Default)]
pub struct TestVisualizer;

impl IdentifiedViewSystem for TestVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "TestVisualizer".into()
    }
}

impl VisualizerSystem for TestVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::empty()
    }

    fn execute(
        &mut self,
        _ctx: &ViewContext<'_>,
        _query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        Ok(VisualizerExecutionOutput::default())
    }
}

/// A minimal view class for testing.
#[derive(Default)]
pub struct TestViewClass;

impl ViewClass for TestViewClass {
    fn identifier() -> ViewClassIdentifier {
        "TestView".into()
    }

    fn display_name(&self) -> &'static str {
        "Test"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_UNKNOWN
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> re_ui::Help {
        re_ui::Help::new("Test view class")
    }

    fn on_register(
        &self,
        system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<TestVisualizer>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<()>::default()
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
        _include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        ViewSpawnHeuristics::root()
    }

    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_chunk_store::MissingChunkReporter,
        _ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        _query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        Ok(())
    }
}
