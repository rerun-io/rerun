use crate::context_menu::ContextMenuItem;
use crate::ViewportBlueprint;
use re_viewer_context::ViewerContext;

/// Group items into a sub-menu
pub(super) struct SubMenu {
    label: String,
    actions: Vec<Box<dyn ContextMenuItem>>,
}

impl SubMenu {
    pub(super) fn item(
        label: &str,
        actions: impl IntoIterator<Item = Box<dyn ContextMenuItem>>,
    ) -> Box<dyn ContextMenuItem> {
        let actions = actions.into_iter().collect();
        Box::new(Self {
            label: label.to_owned(),
            actions,
        })
    }
}

impl ContextMenuItem for SubMenu {
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        ui.menu_button(&self.label, |ui| {
            for action in &self.actions {
                let response = action.ui(ctx, viewport_blueprint, ui);
                if response.clicked() {
                    ui.close_menu();
                }
            }
        })
        .response
    }
}

/// Add a separator to the context menu
pub(super) struct Separator;

impl Separator {
    pub(super) fn item() -> Box<dyn ContextMenuItem> {
        Box::new(Self)
    }
}

impl ContextMenuItem for Separator {
    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _viewport_blueprint: &ViewportBlueprint,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        ui.separator()
    }
}
