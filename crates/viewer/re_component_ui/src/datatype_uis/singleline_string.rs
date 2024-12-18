use re_types::{
    components::{Name, Text},
    datatypes::Utf8,
    Loggable as _,
};
use re_ui::UiExt as _;
use re_viewer_context::{
    external::{
        re_chunk_store::{LatestAtQuery, RowId},
        re_entity_db::EntityDb,
        re_log_types::EntityPath,
    },
    MaybeMutRef, UiLayout, ViewerContext,
};

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
        UiLayout::List.data_label(ui, value.as_str())
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
        UiLayout::SelectionPanel.data_label(ui, value.as_str())
    }
}

// TODO(#6661): Should be merged with edit_singleline_string.
#[allow(clippy::too_many_arguments)]
pub fn display_text_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _path: &EntityPath,
    _row_id: Option<RowId>,
    data: &dyn arrow::array::Array,
) {
    let text = match Text::from_arrow(data) {
        Ok(text) => text.first().cloned(),
        Err(err) => {
            ui.error_label("Failed to deserialize")
                .on_hover_text(err.to_string());
            return;
        }
    };

    let Some(text) = text else {
        ui.weak("(none)");
        return;
    };

    ui_layout.data_label(ui, text);
}

// TODO(#6661): Should be merged with edit_singleline_string.
#[allow(clippy::too_many_arguments)]
pub fn display_name_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _path: &EntityPath,
    _row_id: Option<RowId>,
    data: &dyn arrow::array::Array,
) {
    let name = match Name::from_arrow(data) {
        Ok(name) => name.first().cloned(),
        Err(err) => {
            ui.error_label("Failed to deserialize")
                .on_hover_text(err.to_string());
            return;
        }
    };

    let Some(name) = name else {
        ui.weak("(none)");
        return;
    };

    ui_layout.data_label(ui, name);
}
