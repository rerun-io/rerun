use re_chunk::EntityPath;
use re_log_types::example_components::MyPoint;
use re_ui::Help;
use re_viewer_context::external::re_chunk_store::external::re_chunk;
use re_viewer_context::{
    IdentifiedViewSystem, ViewClass, ViewSpawnHeuristics, ViewState, ViewerContext,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem, suggest_view_for_each_entity,
};

#[derive(Default)]
pub struct TestView;

pub struct TestViewState;

impl ViewState for TestViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TestSystem;

impl VisualizerSystem for TestSystem {
    fn visualizer_query_info(&self) -> re_viewer_context::VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<re_log_types::example_components::MyPoints>()
    }

    fn execute(
        &mut self,
        _ctx: &re_viewer_context::ViewContext<'_>,
        _query: &re_viewer_context::ViewQuery<'_>,
        _context_systems: &re_viewer_context::ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, re_viewer_context::ViewSystemExecutionError> {
        Ok(VisualizerExecutionOutput::default())
    }
}

impl IdentifiedViewSystem for TestSystem {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Test".into()
    }
}

impl ViewClass for TestView {
    fn identifier() -> re_sdk_types::ViewClassIdentifier
    where
        Self: Sized,
    {
        "TestView".into()
    }

    fn display_name(&self) -> &'static str {
        "Test view"
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> re_ui::Help {
        Help::new("Test view").markdown("Only used in tests.")
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), re_viewer_context::ViewClassRegistryError> {
        system_registry.register_visualizer::<TestSystem>()?;

        system_registry
            .register_fallback_provider(MyPoint::partial_descriptor().component, |_ctx| {
                MyPoint::new(0.0, 0.0)
            });

        Ok(())
    }

    fn new_state(&self) -> Box<dyn re_viewer_context::ViewState> {
        Box::new(TestViewState {})
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        suggest_view_for_each_entity::<TestSystem>(ctx, include_entity)
    }

    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn re_viewer_context::ViewState,
        _query: &re_viewer_context::ViewQuery<'_>,
        _system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), re_viewer_context::ViewSystemExecutionError> {
        ui.label("Test view");
        Ok(())
    }
}
