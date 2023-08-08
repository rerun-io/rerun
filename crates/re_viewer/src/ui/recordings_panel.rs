use re_viewer_context::{SystemCommand, SystemCommandSender, ViewerContext};

/// Show the Recordings section of the left panel
pub fn recordings_panel_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    ctx.re_ui.panel_title_bar_with_buttons(
        ui,
        "Recordings",
        Some("These are the Recordings currently loaded in the Viewer"),
        |ui| {
            add_button_ui(ctx, ui);
        },
    );

    let ViewerContext {
        store_context,
        command_sender,
        ..
    } = ctx;

    let store_dbs = store_context.alternate_recordings.clone();

    if store_dbs.is_empty() {
        ui.weak("(empty)");
        return;
    }

    let active_recording = store_context.recording.and_then(|rec| Some(rec.store_id()));

    ui.style_mut().wrap = Some(false);
    for store_db in &store_dbs {
        let info = if let Some(store_info) = store_db.store_info() {
            format!(
                "{} - {}",
                store_info.application_id,
                store_info.started.format()
            )
        } else {
            "<UNKNOWN>".to_owned()
        };
        if ui
            .radio(active_recording == Some(store_db.store_id()), info)
            .clicked()
        {
            command_sender.send_system(SystemCommand::SetRecordingId(store_db.store_id().clone()));
        }
    }
}

fn add_button_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    if ctx
        .re_ui
        .small_icon_button(ui, &re_ui::icons::ADD)
        .on_hover_text("Load a Recording from disk")
        .clicked()
    {
        //TODO(ab)
    }
}
