use re_sdk_types::datatypes::Utf8;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::{MaybeMutRef, UiLayout};

/// Generic singleline string editor.
pub fn edit_singleline_string(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = Utf8>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, Utf8> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_singleline_string_impl(ui, &mut value, false)
}

/// Non monomorphized implementation of [`edit_singleline_string`].
fn edit_singleline_string_impl(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Utf8>,
    is_password: bool,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        let mut edit_name = value.to_string();
        let response = egui::TextEdit::singleline(&mut edit_name)
            .password(is_password)
            .show(ui)
            .response;
        *value = edit_name.into();
        response
    } else {
        UiLayout::List.data_label(ui, SyntaxHighlightedBuilder::new().with_string_value(value))
    }
}

/// Generic multiline string editor.
pub fn edit_multiline_string(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = Utf8>>,
) -> egui::Response {
    let mut value: MaybeMutRef<'_, Utf8> = match value {
        MaybeMutRef::Ref(value) => MaybeMutRef::Ref(value),
        MaybeMutRef::MutRef(value) => MaybeMutRef::MutRef(value),
    };
    edit_multiline_string_impl(ui, &mut value)
}

/// Non monomorphized implementation of [`edit_multiline_string`].
fn edit_multiline_string_impl(
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Utf8>,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        let mut edit_name = value.to_string();
        let response = egui::TextEdit::multiline(&mut edit_name).show(ui).response;
        *value = edit_name.into();
        response
    } else {
        UiLayout::SelectionPanel
            .data_label(ui, SyntaxHighlightedBuilder::new().with_string_value(value))
    }
}
