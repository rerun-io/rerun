use re_types::{
    components::{Name, Text},
    external::arrow2,
    Loggable as _,
};
use re_ui::UiExt as _;
use re_viewer_context::{
    external::{re_chunk_store::LatestAtQuery, re_entity_db::EntityDb, re_log_types::EntityPath},
    UiLayout, ViewerContext,
};

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

// TODO(#6661): Should be merged with edit_singleline_string.
pub fn display_text_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _path: &EntityPath,
    data: &dyn arrow2::array::Array,
) {
    let text = match Text::from_arrow(data) {
        Ok(text) => text.first().cloned(),
        Err(err) => {
            ui.error_label("failed to deserialize")
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
pub fn display_name_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _path: &EntityPath,
    data: &dyn arrow2::array::Array,
) {
    let name = match Name::from_arrow(data) {
        Ok(name) => name.first().cloned(),
        Err(err) => {
            ui.error_label("failed to deserialize")
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
