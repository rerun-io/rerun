use re_sdk_types::datatypes::Uuid;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::{MaybeMutRef, UiLayout};

pub fn view_uuid(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = Uuid>>,
) -> egui::Response {
    UiLayout::List.data_label(
        ui,
        SyntaxHighlightedBuilder::new().with_primitive(&value.as_ref().to_string()),
    )
}
