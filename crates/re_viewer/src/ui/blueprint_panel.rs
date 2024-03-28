use re_viewer_context::{SystemCommandSender as _, ViewerContext};
use re_viewport::Viewport;

/// Show the Blueprint section of the left panel based on the current [`Viewport`]
pub fn blueprint_panel_ui(
    viewport: &mut Viewport<'_, '_>,
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
) {
    ctx.re_ui.panel_content(ui, |_, ui| {
        ctx.re_ui.panel_title_bar_with_buttons(
            ui,
            "Blueprint",
            Some("The blueprint is where you can configure the Rerun Viewer"),
            |ui| {
                viewport.add_new_spaceview_button_ui(ctx, ui);
                reset_blueprint_button_ui(ctx, ui);
            },
        );
    });

    // This call is excluded from `panel_content` because it has a ScrollArea, which should not be
    // inset. Instead, it calls panel_content itself inside the ScrollArea.
    viewport.tree_ui(ctx, ui);
}

fn reset_blueprint_button_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    let hover_text = if ctx.store_context.default_blueprint.is_some() {
        "Reset to the default blueprint for this app"
    } else {
        "Re-populate viewport with automatically chosen space views"
    };

    if ctx
        .re_ui
        .small_icon_button(ui, &re_ui::icons::RESET)
        .on_hover_text(hover_text)
        .clicked()
    {
        ctx.command_sender
            .send_system(re_viewer_context::SystemCommand::ResetBlueprint);
    }
}
