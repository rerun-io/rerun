use re_log_types::Timestamp;
use re_sdk_types::datatypes;
use re_ui::UiLayout;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::MaybeMutRef;

pub fn view_timestamp(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = datatypes::TimeInt>>,
) -> egui::Response {
    let value: &datatypes::TimeInt = value;
    UiLayout::List.data_label(
        ui,
        SyntaxHighlightedBuilder::new()
            .with_primitive(&Timestamp::from(*value).format(ctx.app_options().timestamp_format)),
    )
}
