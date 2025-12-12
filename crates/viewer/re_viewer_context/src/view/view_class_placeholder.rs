use re_chunk::EntityPath;
use re_sdk_types::ViewClassIdentifier;
use re_ui::{Help, UiExt as _};

use crate::{
    SystemExecutionOutput, ViewClass, ViewClassRegistryError, ViewQuery, ViewSpawnHeuristics,
    ViewState, ViewSystemExecutionError, ViewSystemRegistrator, ViewerContext,
};

/// A placeholder view class that can be used when the actual class is not registered.
#[derive(Default)]
pub struct ViewClassPlaceholder;

impl ViewClass for ViewClassPlaceholder {
    fn identifier() -> ViewClassIdentifier {
        "UnknownViewClass".into()
    }

    fn display_name(&self) -> &'static str {
        "Unknown view class"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_UNKNOWN
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Placeholder view").markdown("Placeholder view for unknown view class")
    }

    fn on_register(
        &self,
        _system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        Ok(())
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<()>::default()
    }

    fn layout_priority(&self) -> crate::ViewClassLayoutPriority {
        crate::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
        _include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        ViewSpawnHeuristics::empty()
    }

    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        _query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        let tokens = ui.tokens();
        egui::Frame {
            inner_margin: egui::Margin::same(tokens.view_padding()),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.warning_label("Unknown view class");

            ui.markdown_ui(
                "This happens if either the blueprint specifies an invalid view class or \
                this version of the viewer does not know about this type.\n\n\
                \
                **Note**: some views may require a specific Cargo feature to be enabled. In \
                particular, the map view requires the `map_view` feature.",
            );
        });

        Ok(())
    }
}
