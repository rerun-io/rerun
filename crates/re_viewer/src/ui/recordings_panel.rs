use std::collections::BTreeMap;

use re_log_types::LogMsg;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_viewer_context::{
    AppOptions, CommandSender, SystemCommand, SystemCommandSender, ViewerContext,
};

/// Show the currently open Recordings in a selectable list.
/// Also shows the currently loading receivers.
///
/// Returns `true` if any recordings were shown.
pub fn recordings_panel_ui(
    ctx: &ViewerContext<'_>,
    rx: &ReceiveSet<LogMsg>,
    ui: &mut egui::Ui,
) -> bool {
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
            ctx.re_ui.panel_content(ui, |_re_ui, ui| {
                let mut any_shown = false;
                any_shown |= recording_list_ui(ctx, ui);

                // Show currently loading things after.
                // They will likely end up here as recordings soon.
                any_shown |= loading_receivers_ui(ctx, rx, ui);

                any_shown
            })
        })
        .inner
}

fn loading_receivers_ui(
    ctx: &ViewerContext<'_>,
    rx: &ReceiveSet<LogMsg>,
    ui: &mut egui::Ui,
) -> bool {
    let sources_with_stores: ahash::HashSet<SmartChannelSource> = ctx
        .store_context
        .all_recordings
        .iter()
        .filter_map(|store| store.data_source.clone())
        .collect();

    let mut any_shown = false;

    for source in rx.sources() {
        let string = match source.as_ref() {
            // We only show things we know are very-soon-to-be recordings:
            SmartChannelSource::File(path) => format!("Loading {}…", path.display()),
            SmartChannelSource::RrdHttpStream { url } => format!("Loading {url}…"),

            SmartChannelSource::RrdWebEventListener
            | SmartChannelSource::Sdk
            | SmartChannelSource::WsClient { .. }
            | SmartChannelSource::TcpServer { .. }
            | SmartChannelSource::Stdin => {
                // TODO(#3046): show these in status bar
                continue;
            }
        };

        // Only show if we don't have a recording for this source,
        // i.e. if this source hasn't sent anything yet.
        // Note that usually there is a one-to-one mapping between a source and a recording,
        // but it is possible to send multiple recordings over the same channel.
        if !sources_with_stores.contains(&source) {
            any_shown = true;
            let response = ctx
                .re_ui
                .list_item(string)
                .with_buttons(|re_ui, ui| {
                    let resp = re_ui
                        .small_icon_button(ui, &re_ui::icons::REMOVE)
                        .on_hover_text("Disconnect from this source");
                    if resp.clicked() {
                        rx.remove(&source);
                    }
                    resp
                })
                .show(ui);
            if let SmartChannelSource::TcpServer { .. } = source.as_ref() {
                response.on_hover_text("You can connect to this viewer from a Rerun SDK");
            }
        }
    }

    any_shown
}

/// Draw the recording list.
///
/// Returns `true` if any recordings were shown.
fn recording_list_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) -> bool {
    let ViewerContext {
        store_context,
        command_sender,
        ..
    } = ctx;

    let mut store_dbs_map: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for store_db in &store_context.all_recordings {
        let key = store_db
            .store_info()
            .map_or("<unknown>", |info| info.application_id.as_str());
        store_dbs_map.entry(key).or_default().push(*store_db);
    }

    if store_dbs_map.is_empty() {
        return false;
    }

    for store_dbs in store_dbs_map.values_mut() {
        store_dbs.sort_by_key(|store_db| store_db.store_info().map(|info| info.started));
    }

    let active_recording = store_context.recording.map(|rec| rec.store_id());

    for (app_id, store_dbs) in store_dbs_map {
        if store_dbs.len() == 1 {
            let store_db = store_dbs[0];
            if recording_ui(
                ctx.app_options,
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
                ui.make_persistent_id(app_id),
                true,
                |_, ui| {
                    for store_db in store_dbs {
                        if recording_ui(
                            ctx.app_options,
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

    true
}

/// Show the UI for a single recording.
///
/// If an `app_id_label` is provided, it will be shown in front of the recording time.
fn recording_ui(
    app_options: &AppOptions,
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
            info.started.format_time_custom(
                "[hour]:[minute]:[second]",
                app_options.time_zone_for_timestamps,
            )
        })
        .unwrap_or("<unknown time>".to_owned());

    let response = re_ui
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
        .show(ui);

    response.on_hover_ui(|ui| {
        recording_hover_ui(app_options, re_ui, ui, store_db);
    })
}

fn recording_hover_ui(
    app_options: &AppOptions,
    re_ui: &re_ui::ReUi,
    ui: &mut egui::Ui,
    store_db: &re_data_store::StoreDb,
) {
    egui::Grid::new("recording_hover_ui")
        .num_columns(2)
        .show(ui, |ui| {
            re_ui.grid_left_hand_label(ui, "Store ID");
            ui.label(store_db.store_id().to_string());
            ui.end_row();

            if let Some(data_source) = &store_db.data_source {
                re_ui.grid_left_hand_label(ui, "Data source");
                ui.label(data_source_string(data_source));
                ui.end_row();
            }

            if let Some(store_info) = store_db.store_info() {
                let re_log_types::StoreInfo {
                    application_id,
                    store_id: _,
                    is_official_example: _,
                    started,
                    store_source,
                    store_kind,
                } = store_info;

                re_ui.grid_left_hand_label(ui, "Application ID");
                ui.label(application_id.to_string());
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Recording started");
                ui.label(started.format(app_options.time_zone_for_timestamps));
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Source");
                ui.label(store_source.to_string());
                ui.end_row();

                // We are in the recordings menu, we know the kind
                if false {
                    re_ui.grid_left_hand_label(ui, "Kind");
                    ui.label(store_kind.to_string());
                    ui.end_row();
                }
            }
        });
}

fn data_source_string(data_source: &re_smart_channel::SmartChannelSource) -> String {
    match data_source {
        SmartChannelSource::File(path) => path.display().to_string(),
        SmartChannelSource::RrdHttpStream { url } => url.clone(),
        SmartChannelSource::RrdWebEventListener => "Web Event Listener".to_owned(),
        SmartChannelSource::Sdk => "SDK".to_owned(),
        SmartChannelSource::WsClient { ws_server_url } => ws_server_url.clone(),
        SmartChannelSource::TcpServer { port } => format!("TCP Server, port {port}"),
        SmartChannelSource::Stdin => "Standard Input".to_owned(),
    }
}

fn add_button_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
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
