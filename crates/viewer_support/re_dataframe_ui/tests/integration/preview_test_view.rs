use re_sdk_types::ViewClassIdentifier;
use re_sdk_types::blueprint::archetypes::VisualBounds2D;
use re_viewer_context::{
    MissingChunkReporter, SystemExecutionOutput, ViewClass, ViewClassRegistryError,
    ViewClassUiOutput, ViewQuery, ViewSpawnContext, ViewSpawnHeuristics, ViewState,
    ViewStateExt as _, ViewSystemExecutionError, ViewSystemRegistrator, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

#[derive(Default)]
pub struct PreviewTestView;

/// State allows injecting an aspect ratio.
#[derive(Default)]
struct State {
    aspect_ratio: Option<f32>,
}

impl ViewState for State {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl ViewClass for PreviewTestView {
    fn identifier() -> ViewClassIdentifier {
        "PreviewTest".into()
    }

    fn display_name(&self) -> &'static str {
        "Preview test"
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> re_ui::Help {
        re_ui::Help::new_without_title()
    }

    fn on_register(
        &self,
        _system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        Ok(())
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<State>::default()
    }

    fn preferred_tile_aspect_ratio(&self, state: &dyn ViewState) -> Option<f32> {
        state
            .downcast_ref::<State>()
            .expect("preview state")
            .aspect_ratio
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
        _spawn_context: ViewSpawnContext,
    ) -> ViewSpawnHeuristics {
        ViewSpawnHeuristics::empty()
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<ViewClassUiOutput, ViewSystemExecutionError> {
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            0.0,
            #[expect(clippy::disallowed_methods)] // Just a color for testing.
            egui::Color32::from_rgb(55, 85, 110),
        );

        // Learn the ratio during rendering, as data-dependent views do after loading.
        state.downcast_mut::<State>()?.aspect_ratio =
            ViewProperty::from_archetype_for_view::<VisualBounds2D>(ctx, query.view_id)
                .component_or_empty::<re_sdk_types::blueprint::components::VisualBounds2D>(
                    VisualBounds2D::descriptor_range().component,
                )
                .expect("valid visual bounds")
                .map(|bounds| (bounds.x_range.abs_len() / bounds.y_range.abs_len()) as f32);

        Ok(Default::default())
    }
}
