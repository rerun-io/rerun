use re_log_types::Timestamp;
use re_sdk_types::encodings;
use re_ui::UiLayout;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::MaybeMutRef;

pub fn view_timestamp(
    ctx: &re_viewer_context::AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, impl std::ops::DerefMut<Target = encodings::TimeInt>>,
) -> egui::Response {
    let value: &encodings::TimeInt = value;
    UiLayout::List.data_label(
        ui,
        SyntaxHighlightedBuilder::new()
            .with_primitive(&Timestamp::from(*value).format(ctx.app_options.timestamp_format)),
    )
}
