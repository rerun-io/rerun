use std::collections::BTreeMap;

use re_data_ui::{item_ui::entity_db_button_ui, DataUi as _};
use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, LogMsg, StoreKind};
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_types::components::Timestamp;
use re_ui::{icons, UiExt as _};
use re_viewer_context::{
    Item, StoreHub, SystemCommand, SystemCommandSender as _, UiLayout, ViewerContext,
};

use crate::app_state::WelcomeScreenState;

/// Show the currently open Recordings in a selectable list.
/// Also shows the currently loading receivers.
pub fn recordings_panel_ui(
    ctx: &ViewerContext<'_>,
    rx: &ReceiveSet<LogMsg>,
    ui: &mut egui::Ui,
    welcome_screen_state: &WelcomeScreenState,
) {
    ui.panel_content(|ui| {
        ui.panel_title_bar_with_buttons(
            "Recordings",
            Some(
                "These are the Recordings currently loaded in the Viewer, organized by application",
            ),
            |ui| {
                add_button_ui(ctx, ui);
            },
        );
    });

    egui::ScrollArea::both()
        .id_salt("recordings_scroll_area")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.panel_content(|ui| {
                re_ui::list_item::list_item_scope(ui, "recording panel", |ui| {
                    recording_list_ui(ctx, ui, welcome_screen_state);

                    // Show currently loading things after.
                    // They will likely end up here as recordings soon.
                    loading_receivers_ui(ctx, rx, ui);
                });
            });
        });
}

fn loading_receivers_ui(ctx: &ViewerContext<'_>, rx: &ReceiveSet<LogMsg>, ui: &mut egui::Ui) {
    let sources_with_stores: ahash::HashSet<SmartChannelSource> = ctx
        .store_context
        .bundle
        .recordings()
        .filter_map(|store| store.data_source.clone())
        .collect();

    for source in rx.sources() {
        let string = match source.as_ref() {
            // We only show things we know are very-soon-to-be recordings:
            SmartChannelSource::File(path) => format!("Loading {}…", path.display()),
            SmartChannelSource::RrdHttpStream { url, .. }
            | SmartChannelSource::RedapGrpcStream { url } => format!("Loading {url}…"),

            SmartChannelSource::RrdWebEventListener
            | SmartChannelSource::JsChannel { .. }
            | SmartChannelSource::MessageProxy { .. }
            | SmartChannelSource::Sdk
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
            // never more than one level deep
            let response = ui.list_item().show_flat(
                ui,
                re_ui::list_item::LabelContent::new(string).with_buttons(|ui| {
                    let resp = ui
                        .small_icon_button(&re_ui::icons::REMOVE)
                        .on_hover_text("Disconnect from this source");
                    if resp.clicked() {
                        rx.remove(&source);
                    }
                    resp
                }),
            );
            if let SmartChannelSource::MessageProxy { .. } = source.as_ref() {
                response.on_hover_text("You can connect to this viewer from a Rerun SDK");
            }
        }
    }
}

/// Draw the recording list.
fn recording_list_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    welcome_screen_state: &WelcomeScreenState,
) {
    let mut entity_dbs_map: BTreeMap<ApplicationId, Vec<&EntityDb>> = BTreeMap::new();

    // Always have a place for the welcome screen, even if there is no recordings or blueprints associated with it:
    entity_dbs_map
        .entry(StoreHub::welcome_screen_app_id())
        .or_default();

    for entity_db in ctx.store_context.bundle.entity_dbs() {
        // We want to show all open applications, even if they have no recordings
        let Some(app_id) = entity_db.app_id().cloned() else {
            continue; // this only happens if we haven't even started loading it, or if something is really wrong with it.
        };
        let recordings = entity_dbs_map.entry(app_id).or_default();

        if entity_db.store_kind() == StoreKind::Recording {
            recordings.push(entity_db);
        }
    }

    if let Some(entity_dbs) = entity_dbs_map.remove(&StoreHub::welcome_screen_app_id()) {
        // Always show welcome screen first, if at all:
        if ctx
            .app_options()
            .include_welcome_screen_button_in_recordings_panel
            && !welcome_screen_state.hide
        {
            debug_assert!(
                entity_dbs.is_empty(),
                "There shouldn't be any recording for the welcome screen, but there are!"
            );
            app_and_its_recordings_ui(
                ctx,
                ui,
                &StoreHub::welcome_screen_app_id(),
                Default::default(),
            );
        }
    }

    if entity_dbs_map.is_empty() && welcome_screen_state.hide {
        ui.list_item().interactive(false).show_flat(
            ui,
            re_ui::list_item::LabelContent::new("No recordings loaded")
                .weak(true)
                .italics(true),
        );
    }

    for (app_id, entity_dbs) in entity_dbs_map {
        app_and_its_recordings_ui(ctx, ui, &app_id, entity_dbs);
    }
}

fn app_and_its_recordings_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    app_id: &ApplicationId,
    mut entity_dbs: Vec<&EntityDb>,
) {
    entity_dbs.sort_by_key(|entity_db| entity_db.recording_property::<Timestamp>());

    let app_item = Item::AppId(app_id.clone());
    let selected = ctx.selection().contains_item(&app_item);

    let app_list_item = ui.list_item().selected(selected);
    let app_list_item_content = re_ui::list_item::LabelContent::new(app_id.to_string())
        .with_icon_fn(|ui, rect, visuals| {
            // Color icon based on whether this is the active application or not:
            let color = if &ctx.store_context.app_id == app_id {
                visuals.fg_stroke.color
            } else {
                ui.visuals().widgets.noninteractive.fg_stroke.color
            };
            icons::APPLICATION.as_image().tint(color).paint_at(ui, rect);
        });

    let item_response = if app_id == &StoreHub::welcome_screen_app_id() {
        // Special case: the welcome screen never has any recordings
        debug_assert!(
            entity_dbs.is_empty(),
            "There shouldn't be any recording for the welcome screen, but there are!"
        );
        app_list_item.show_hierarchical(ui, app_list_item_content)
    } else {
        // Normal application
        let id = ui.make_persistent_id(app_id);
        let app_list_item_content = app_list_item_content.with_buttons(|ui| {
            // Close-button:
            let resp = ui.small_icon_button(&icons::REMOVE).on_hover_text(
                "Close this application and all its recordings. This cannot be undone.",
            );
            if resp.clicked() {
                ctx.command_sender()
                    .send_system(SystemCommand::CloseApp(app_id.clone()));
            }
            resp
        });
        app_list_item
            .show_hierarchical_with_children(ui, id, true, app_list_item_content, |ui| {
                // Show all the recordings for this application:
                if entity_dbs.is_empty() {
                    ui.weak("(no recordings)").on_hover_ui(|ui| {
                        ui.label("No recordings loaded for this application");
                    });
                } else {
                    for entity_db in entity_dbs {
                        let include_app_id = false; // we already show it in the parent
                        entity_db_button_ui(
                            ctx,
                            ui,
                            entity_db,
                            UiLayout::SelectionPanel,
                            include_app_id,
                        );
                    }
                }
            })
            .item_response
    };

    let item_response = item_response.on_hover_ui(|ui| {
        app_id.data_ui_recording(ctx, ui, UiLayout::Tooltip);
    });

    ctx.handle_select_hover_drag_interactions(&item_response, app_item, false);

    if item_response.clicked() {
        // Switch to this application:
        ctx.command_sender()
            .send_system(re_viewer_context::SystemCommand::ActivateApp(
                app_id.clone(),
            ));
    }
}

fn add_button_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    use re_ui::UICommandSender as _;

    if ui
        .small_icon_button(&re_ui::icons::ADD)
        .on_hover_text(re_ui::UICommand::Open.tooltip_with_shortcut(ui.ctx()))
        .clicked()
    {
        ctx.command_sender().send_ui(re_ui::UICommand::Open);
    }
}
