use re_blueprint_tree::BlueprintTree;
use re_viewer_context::{SystemCommandSender as _, ViewerContext};
use re_viewport_blueprint::ViewportBlueprint;

/// Show the Blueprint section of the left panel based on the current [`ViewportBlueprint`]
pub fn blueprint_panel_ui(
    blueprint_tree: &mut BlueprintTree,
    ctx: &ViewerContext<'_>,
    blueprint: &ViewportBlueprint,
    ui: &mut egui::Ui,
) {
    ctx.re_ui.panel_content(ui, |_, ui| {
        ctx.re_ui.panel_title_bar_with_buttons(
            ui,
            "Blueprint",
            Some("The blueprint is where you can configure the Rerun Viewer"),
            |ui| {
                blueprint_tree.add_new_spaceview_button_ui(ctx, blueprint, ui);
                reset_blueprint_button_ui(ctx, ui);
            },
        );
    });

    // This call is excluded from `panel_content` because it has a ScrollArea, which should not be
    // inset. Instead, it calls panel_content itself inside the ScrollArea.
    blueprint_tree.tree_ui(ctx, blueprint, ui);
}

fn reset_blueprint_button_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    let default_blueprint_id = ctx
        .store_context
        .hub
        .default_blueprint_id_for_app(&ctx.store_context.app_id);

    let default_blueprint = default_blueprint_id.and_then(|id| ctx.store_context.bundle.get(id));

    let mut disabled_reason = None;

    if let Some(default_blueprint) = default_blueprint {
        let active_is_clone_of_default =
            Some(default_blueprint.store_id()) == ctx.store_context.blueprint.cloned_from();
        let last_modified_at_the_same_time =
            default_blueprint.latest_row_id() == ctx.store_context.blueprint.latest_row_id();
        if active_is_clone_of_default && last_modified_at_the_same_time {
            disabled_reason = Some("No modifications has been made");
        }
    }

    let enabled = disabled_reason.is_none();
    let response = ui.add_enabled(
        enabled,
        ctx.re_ui.small_icon_button_widget(ui, &re_ui::icons::RESET),
    );

    let response = if let Some(disabled_reason) = disabled_reason {
        response.on_disabled_hover_text(disabled_reason)
    } else {
        let hover_text = if default_blueprint_id.is_some() {
            "Reset to the default blueprint for this app"
        } else {
            "Re-populate viewport with automatically chosen space views"
        };
        response.on_hover_text(hover_text)
    };

    if response.clicked() {
        ctx.command_sender
            .send_system(re_viewer_context::SystemCommand::ClearActiveBlueprint);
    }
}
