use re_viewer_context::{CommandSender, SystemCommand, SystemCommandSender, ViewerContext};
use std::collections::BTreeMap;
use time::macros::format_description;

static TIME_FORMAT_DESCRIPTION: once_cell::sync::Lazy<
    &'static [time::format_description::FormatItem<'static>],
> = once_cell::sync::Lazy::new(|| format_description!(version = 2, "[hour]:[minute]:[second]Z"));

/// Show the currently open Recordings in a selectable list.
pub fn recordings_panel_ui(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
    ctx.re_ui.panel_content(ui, |re_ui, ui| {
        re_ui.panel_title_bar_with_buttons(
            ui,
            "Recordings",
            Some("These are the Recordings currently loaded in the Viewer"),
            |ui| {
                add_button_ui(ctx, ui);
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

    let mut store_dbs_map: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for store_db in &store_context.alternate_recordings {
        let key = store_db
            .store_info()
            .map_or("<unknown>", |info| info.application_id.as_str());
        store_dbs_map.entry(key).or_default().push(*store_db);
    }

    if store_dbs_map.is_empty() {
        return;
    }

    for store_dbs in store_dbs_map.values_mut() {
        store_dbs.sort_by_key(|store_db| store_db.store_info().map(|info| info.started));
    }

    let active_recording = store_context.recording.map(|rec| rec.store_id());

    for (app_id, store_dbs) in store_dbs_map {
        if store_dbs.len() == 1 {
            let store_db = store_dbs[0];
            if recording_ui(
                ctx.re_ui,
                ui,
                store_db,
                Some(app_id),
                active_recording,
                command_sender,
            )
            .clicked()
            {
                command_sender
                    .send_system(SystemCommand::SetRecordingId(store_db.store_id().clone()));
            }
        } else {
            ctx.re_ui.list_item(app_id).active(false).show_collapsing(
                ui,
                ui.id().with(app_id),
                true,
                |_, ui| {
                    for store_db in store_dbs {
                        if recording_ui(
                            ctx.re_ui,
                            ui,
                            store_db,
                            None,
                            active_recording,
                            command_sender,
                        )
                        .clicked()
                        {
                            command_sender.send_system(SystemCommand::SetRecordingId(
                                store_db.store_id().clone(),
                            ));
                        }
                    }
                },
            );
        }
    }
}

/// Show the UI for a single recording.
///
/// If an `app_id_label` is provided, it will be shown in front of the recording time.
fn recording_ui(
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    store_db: &re_data_store::StoreDb,
    app_id_label: Option<&str>,
    active_recording: Option<&re_log_types::StoreId>,
    command_sender: &CommandSender,
) -> egui::Response {
    let prefix = if let Some(app_id_label) = app_id_label {
        format!("{app_id_label} - ")
    } else {
        String::new()
    };

    let name = store_db
        .store_info()
        .and_then(|info| {
            info.started
                .to_datetime()
                .and_then(|dt| dt.format(&TIME_FORMAT_DESCRIPTION).ok())
        })
        .unwrap_or("<unknown time>".to_owned());

    re_ui
        .list_item(format!("{prefix}{name}"))
        .with_buttons(|re_ui, ui| {
            let resp = re_ui
                .small_icon_button(ui, &re_ui::icons::REMOVE)
                .on_hover_text("Close this Recording (unsaved data will be lost)");
            if resp.clicked() {
                command_sender
                    .send_system(SystemCommand::CloseRecordingId(store_db.store_id().clone()));
            }
            resp
        })
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
}

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
