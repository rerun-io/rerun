use egui::{Response, Ui};

use crate::{ContextMenuAction, ContextMenuContext};

/// Group items into a sub-menu
pub(super) struct SubMenu {
    pub label: String,
    pub actions: Vec<Box<dyn ContextMenuAction + Sync + Send>>,
}

impl ContextMenuAction for SubMenu {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        // We need at least one sub-action to support the selection to go ahead and show the sub-menu
        self.actions
            .iter()
            .any(|action| action.supports_selection(ctx))
    }

    fn process_selection(&self, ctx: &ContextMenuContext<'_>) {
        self.actions
            .iter()
            .for_each(|action| action.process_selection(ctx));
    }

    fn ui(&self, ctx: &ContextMenuContext<'_>, ui: &mut Ui) -> Response {
        ui.menu_button(&self.label, |ui| {
            for action in &self.actions {
                if !action.supports_selection(ctx) {
                    continue;
                }

                let response = action.ui(ctx, ui);
                if response.clicked() {
                    ui.close();
                }
            }
        })
        .response
    }
}
