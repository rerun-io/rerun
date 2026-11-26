pub trait ResponseExt {
    /// Like [`egui::Response::on_hover_ui`], but the tooltip stays open while the user scrolls
    /// inside it, so `add_contents` can contain an [`egui::ScrollArea`].
    // Egui 0.37 suppresses tooltips after scrolling, including scrolls inside the tooltip.
    // This helper keeps a hovered tooltip open so its ScrollArea remains usable.
    // See https://github.com/emilk/egui/issues/4781.
    fn on_scrollable_hover_ui(self, add_contents: impl FnOnce(&mut egui::Ui)) -> Self;
}

impl ResponseExt for egui::Response {
    fn on_scrollable_hover_ui(self, add_contents: impl FnOnce(&mut egui::Ui)) -> Self {
        let mut tooltip = egui::Tooltip::for_enabled(&self);
        if !tooltip.popup.is_open() && is_pointer_in_tooltip(&self.ctx, self.id) {
            tooltip.popup = tooltip.popup.open(true);
        }
        tooltip.show(add_contents);
        self
    }
}

/// Was the tooltip of `widget_id` open last frame, with the pointer now inside it?
fn is_pointer_in_tooltip(ctx: &egui::Context, widget_id: egui::Id) -> bool {
    let Some(pointer) = ctx.pointer_hover_pos() else {
        return false;
    };
    egui::Tooltip::was_tooltip_open_last_frame(ctx, widget_id)
        && egui::AreaState::load(ctx, egui::Tooltip::tooltip_id(widget_id, 0))
            .is_some_and(|area| area.rect().contains(pointer))
}
