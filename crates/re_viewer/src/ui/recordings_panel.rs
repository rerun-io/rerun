use std::collections::BTreeMap;

use re_data_ui::DataUi;
use re_log_types::LogMsg;
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_viewer_context::{SystemCommand, SystemCommandSender, ViewerContext};

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
                // These show up in the top panel - see `top_panel.rs`.
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
                .show(ui, re_ui::list_item::IndentMode::Flat); // never more than one level deep
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
    let mut entity_dbs_map: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for entity_db in &ctx.store_context.all_recordings {
        let key = entity_db
            .store_info()
            .map_or("<unknown>", |info| info.application_id.as_str());
        entity_dbs_map.entry(key).or_default().push(*entity_db);
    }

    if entity_dbs_map.is_empty() {
        return false;
    }

    for entity_dbs in entity_dbs_map.values_mut() {
        entity_dbs.sort_by_key(|entity_db| entity_db.store_info().map(|info| info.started));
    }

    let active_recording = ctx.store_context.recording.map(|rec| rec.store_id());

    for (app_id, entity_dbs) in entity_dbs_map {
        if entity_dbs.len() == 1 {
            let entity_db = entity_dbs[0];
            recording_ui(ctx, ui, entity_db, Some(app_id), active_recording);
        } else {
            ctx.re_ui.list_item(app_id).active(false).show_collapsing(
                ui,
                ui.make_persistent_id(app_id),
                true,
                |_, ui| {
                    for entity_db in entity_dbs {
                        recording_ui(ctx, ui, entity_db, None, active_recording);
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
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entity_db: &re_entity_db::EntityDb,
    app_id_label: Option<&str>,
    active_recording: Option<&re_log_types::StoreId>,
) {
    let app_id_label =
        app_id_label.map_or(String::new(), |app_id_label| format!("{app_id_label} - "));

    let time = entity_db
        .store_info()
        .and_then(|info| {
            info.started
                .format_time_custom("[hour]:[minute]:[second]", ctx.app_options.time_zone)
        })
        .unwrap_or("<unknown time>".to_owned());

    let title = format!("{app_id_label}{time}");

    let store_id = entity_db.store_id().clone();
    let item = re_viewer_context::Item::StoreId(store_id.clone());

    let mut list_item = ctx
        .re_ui
        .list_item(title)
        .selected(ctx.selection().contains_item(&item))
        .with_icon_fn(|_re_ui, ui, rect, visuals| {
            let color = if active_recording == Some(&store_id) {
                visuals.fg_stroke.color
            } else {
                ui.visuals().widgets.noninteractive.fg_stroke.color
            };

            re_ui::icons::STORE
                .as_image()
                .tint(color)
                .paint_at(ui, rect);
        })
        .with_buttons(|re_ui, ui| {
            // Close-button:
            let resp = re_ui
                .small_icon_button(ui, &re_ui::icons::REMOVE)
                .on_hover_text("Close this Recording (unsaved data will be lost)");
            if resp.clicked() {
                ctx.command_sender
                    .send_system(SystemCommand::CloseRecordingId(store_id.clone()));
            }
            resp
        });

    if ctx.hovered().contains_item(&item) {
        list_item = list_item.force_hovered(true);
    }

    let response = list_item
        .show(ui, re_ui::list_item::IndentMode::Flat) // never more than one level deep
        .on_hover_ui(|ui| {
            entity_db.data_ui(
                ctx,
                ui,
                re_viewer_context::UiVerbosity::Full,
                &ctx.current_query(),
                entity_db.store(),
            );
        });

    if response.hovered() {
        ctx.selection_state().set_hovered(item.clone());
    }

    if response.clicked() {
        // Open the recording…
        ctx.command_sender
            .send_system(SystemCommand::SetRecordingId(store_id.clone()));

        // …and select the recording in the recording.
        // Note that we must do it in this order, since the selection state is stored in the recording.
        // That's also why we use a command to set the selection.
        ctx.command_sender
            .send_system(SystemCommand::SetSelection(store_id, item));
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
