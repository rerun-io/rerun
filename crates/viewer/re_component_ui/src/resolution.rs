use re_sdk_types::components::Resolution;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_viewer_context::{MaybeMutRef, StoreViewContext, UiLayout};

pub fn edit_or_view_resolution(
    _ctx: &StoreViewContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Resolution>,
) -> egui::Response {
    // TODO(#6743): Don't allow editing resolution for now since it's part of the pinhole and thus the transform hierarchy which doesn't yet support overrides.
    let [x, y] = value.as_ref().0.0;
    UiLayout::List.data_label(
        ui,
        SyntaxHighlightedBuilder::new()
            .with(&x)
            .with_syntax(" × ")
            .with(&y),
    )
}
