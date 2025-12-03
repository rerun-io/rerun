use egui::{InnerResponse, Popup, Response, SetOpenCommand};

use crate::menu::menu_style;

pub trait ResponseExt {
    fn _self(&self) -> &Response;

    /// Is this response or any child widgets hovered?
    ///
    /// Will return false if some other area is covering the given layer, or if anything is being
    /// dragged.
    ///
    /// This calls [`egui::Context::rect_contains_pointer`]. See also [`Response::hovered`].
    fn container_hovered(&self) -> bool {
        self._self().ctx.dragged_id().is_none() && self.container_contains_pointer()
    }

    /// Does this response or any child widgets contain the mouse pointer?
    ///
    /// Will return false if some other area is covering the given layer.
    ///
    /// This calls [`egui::Context::rect_contains_pointer`]. See also [`Response::contains_pointer`].
    fn container_contains_pointer(&self) -> bool {
        self._self()
            .ctx
            .rect_contains_pointer(self._self().layer_id, self._self().interact_rect)
    }

    /// Were this container or any of its child widgets clicked?
    fn container_clicked(&self) -> bool {
        self.container_contains_pointer() && self._self().ctx.input(|i| i.pointer.primary_clicked())
    }

    /// Were this container or any of its child widgets secondary clicked?
    ///
    /// Does not check for long-press right now (since egui doesn't publicly expose that information).
    fn container_secondary_clicked(&self) -> bool {
        self.container_contains_pointer()
            && self._self().ctx.input(|i| i.pointer.secondary_clicked())
    }

    /// Show a context menu on right clicks anywhere within this widget, even if covered by a click
    /// sensing widget on the same layer.
    fn container_context_menu<R>(
        &self,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<InnerResponse<R>> {
        Popup::menu(self._self())
            .open_memory(if self.container_secondary_clicked() {
                Some(SetOpenCommand::Bool(true))
            } else if self._self().container_clicked() {
                // Explicitly close the menu if the widget was clicked
                // Without this, the context menu would stay open if the user clicks the widget
                Some(SetOpenCommand::Bool(false))
            } else {
                None
            })
            .style(menu_style())
            .at_pointer_fixed()
            .show(add_contents)
    }
}

impl ResponseExt for Response {
    fn _self(&self) -> &Response {
        self
    }
}
