use re_data_store::StoreDb;
use re_viewer_context::{SystemCommand, SystemCommandSender, ViewerContext};
use time::macros::format_description;

/// Show the currently open Recordings in a selectable list.
pub fn recordings_panel_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    ctx.re_ui.panel_content(ui, |re_ui, ui| {
        re_ui.panel_title_bar_with_buttons(
            ui,
            "Recordings",
            Some("These are the Recordings currently loaded in the Viewer"),
            |_ui| {
                #[cfg(not(target_arch = "wasm32"))]
                add_button_ui(ctx, _ui);
            },
        );
    });

    egui::ScrollArea::both()
        .id_source("recordings_scroll_area")
        .auto_shrink([false, true])
        .max_height(300.)
        .show(ui, |ui| {
            ctx.re_ui
                .panel_content(ui, |_re_ui, ui| recording_list_ui(ctx, ui));
        });
}

#[allow(clippy::blocks_in_if_conditions)]
fn recording_list_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    let ViewerContext {
        store_context,
        command_sender,
        ..
    } = ctx;

    let mut store_dbs = store_context.alternate_recordings.clone();
    if store_dbs.is_empty() {
        return;
    }

    fn store_db_key(store_db: &StoreDb) -> impl Ord + '_ {
        store_db
            .store_info()
            .map(|info| (info.application_id.0.as_str(), info.started))
    }

    store_dbs.sort_by_key(|store_db| store_db_key(store_db));

    let active_recording = store_context.recording.map(|rec| rec.store_id());

    let desc = format_description!(version = 2, "[hour]:[minute]:[second]Z");
    for store_db in &store_dbs {
        let info = if let Some(store_info) = store_db.store_info() {
            format!(
                "{} - {}",
                store_info.application_id,
                store_info
                    .started
                    .to_datetime()
                    .and_then(|dt| dt.format(&desc).ok())
                    .unwrap_or("<unknown>".to_owned())
            )
        } else {
            "<UNKNOWN>".to_owned()
        };

        if ctx
            .re_ui
            .list_item(info)
            .with_icon_fn(|_re_ui, ui, rect, visuals| {
                let color = if active_recording == Some(store_db.store_id()) {
                    visuals.fg_stroke.color
                } else {
                    ui.visuals().widgets.noninteractive.fg_stroke.color
                };

                ui.painter()
                    .circle(rect.center(), 4.0, color, egui::Stroke::NONE);
            })
            .show(ui)
            .clicked()
        {
            command_sender.send_system(SystemCommand::SetRecordingId(store_db.store_id().clone()));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn add_button_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    use re_ui::UICommandSender;

    if ctx
        .re_ui
        .small_icon_button(ui, &re_ui::icons::ADD)
        .on_hover_text(re_ui::UICommand::Open.tooltip_with_shortcut(ui.ctx()))
        .clicked()
    {
        ctx.command_sender.send_ui(re_ui::UICommand::Open);
    }
}
