pub trait ContextExt {
    fn _self(&self) -> &egui::Context;

    /// Is the currently focused widget a text edit?
    fn text_edit_focused(&self) -> bool {
        if let Some(id) = self._self().memory(|mem| mem.focused()) {
            egui::text_edit::TextEditState::load(self._self(), id).is_some()
        } else {
            false
        }
    }
}

impl ContextExt for egui::Context {
    fn _self(&self) -> &egui::Context {
        self
    }
}
