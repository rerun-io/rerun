use re_ui::UiExt as _;
use re_viewer_context::MaybeMutRef;

/// Generic editor for a boolean value.
pub fn edit_bool(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = re_sdk_types::datatypes::Bool>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, bool> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(&mut value.deref_mut().0),
    };
    edit_bool_impl(ui, &mut value)
}

/// Non monomorphized implementation of [`edit_bool`].
fn edit_bool_impl(ui: &mut egui::Ui, value: &mut MaybeMutRef<'_, bool>) -> egui::Response {
    match value {
        MaybeMutRef::Ref(value) => {
            // Show a disabled checkbox for immutable values
            let mut value_copy: bool = **value;
            ui.add_enabled_ui(false, |ui| ui.re_checkbox(&mut value_copy, ""))
                .inner
        }
        MaybeMutRef::MutRef(value) => ui.re_checkbox(value, ""),
    }
}
