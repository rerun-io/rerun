/// Generic singleline string editor.
pub fn edit_singleline_string(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut impl std::ops::DerefMut<Target = re_types::datatypes::Utf8>,
) -> egui::Response {
    edit_singleline_string_impl(ui, value)
}

/// Non monomorphized implementation of [`edit_singleline_string`].
fn edit_singleline_string_impl(
    ui: &mut egui::Ui,
    value: &mut re_types::datatypes::Utf8,
) -> egui::Response {
    let mut edit_name = value.to_string();
    let response = egui::TextEdit::singleline(&mut edit_name).show(ui).response;
    *value = edit_name.into();
    response
}
