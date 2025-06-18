use re_data_ui::item_ui::table_id_button_ui;
use re_log_types::LogMsg;
use re_redap_browser::{
    EXAMPLES_ORIGIN, EntryKind, LOCAL_ORIGIN, RedapServers, dataset_and_its_recordings_ui,
};
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_ui::list_item::ItemMenuButton;
use re_ui::{UiExt as _, UiLayout, list_item};
use re_viewer_context::{
    DisplayMode, Item, SystemCommand, SystemCommandSender as _, ViewerContext,
};

use crate::app_state::WelcomeScreenState;

/// Show the currently open Recordings in a selectable list.
/// Also shows the currently loading receivers.
pub fn recordings_panel_ui(
    ctx: &ViewerContext<'_>,
    rx: &ReceiveSet<LogMsg>,
    ui: &mut egui::Ui,
    welcome_screen_state: &WelcomeScreenState,
    servers: &RedapServers,
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
                    recording_list_ui(ctx, ui, welcome_screen_state, servers);

                    // Show currently loading things after.
                    // They will likely end up here as recordings soon.
                    loading_receivers_ui(ctx, rx, ui);
                });
            });
        });
}

fn loading_receivers_ui(ctx: &ViewerContext<'_>, rx: &ReceiveSet<LogMsg>, ui: &mut egui::Ui) {
    let sources_with_stores: ahash::HashSet<SmartChannelSource> = ctx
        .storage_context
        .bundle
        .recordings()
        .filter_map(|store| store.data_source.clone())
        .collect();

    for source in rx.sources() {
        let string = match source.as_ref() {
            // We only show things we know are very-soon-to-be recordings:
            SmartChannelSource::File(path) => format!("Loading {}…", path.display()),
            SmartChannelSource::RrdHttpStream { url, .. } => format!("Loading {url}…"),
            SmartChannelSource::RedapGrpcStream { uri, .. } => format!("Loading {uri}…"),

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
                        .small_icon_button(&re_ui::icons::REMOVE, "Disconnect")
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
    servers: &RedapServers,
) {
    let re_entity_db::SortDatasetsResults {
        remote_recordings,
        example_recordings,
        local_recordings,
    } = ctx.storage_context.bundle.sort_recordings_by_class();

    servers.server_list_ui(ctx, ui, remote_recordings);

    // Show placeholder message if there's absolutely nothing else to show.
    if ctx.storage_context.tables.is_empty()
        && servers.is_empty()
        && local_recordings.is_empty()
        && welcome_screen_state.hide
    {
        ui.list_item().interactive(false).show_flat(
            ui,
            re_ui::list_item::LabelContent::new("No recordings loaded")
                .weak(true)
                .italics(true),
        );
    }

    if (!local_recordings.is_empty() || !ctx.storage_context.tables.is_empty())
        && ui
            .list_item()
            .header()
            .show_hierarchical_with_children(
                ui,
                egui::Id::new("local items"),
                true,
                list_item::LabelContent::header("Local"),
                |ui| {
                    for (app_id, entity_dbs) in local_recordings {
                        dataset_and_its_recordings_ui(
                            ui,
                            ctx,
                            &EntryKind::Local(app_id.clone()),
                            entity_dbs,
                        );
                    }
                    for table_id in ctx.storage_context.tables.keys() {
                        table_id_button_ui(ctx, ui, table_id, UiLayout::SelectionPanel);
                    }
                },
            )
            .item_response
            .clicked()
    {
        ctx.command_sender()
            .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(
                LOCAL_ORIGIN.clone(),
            )));
    }

    // Always show welcome screen last, if at all:
    if (ctx
        .app_options()
        .include_welcome_screen_button_in_recordings_panel
        && !welcome_screen_state.hide)
        || !example_recordings.is_empty()
    {
        let item = Item::RedapServer(EXAMPLES_ORIGIN.clone());
        let selected = ctx.selection().contains_item(&item);
        let list_item = ui.list_item().header().selected(selected);
        let title = list_item::LabelContent::header("Rerun examples");
        let response = if example_recordings.is_empty() {
            list_item.show_flat(ui, title)
        } else {
            list_item
                .show_hierarchical_with_children(
                    ui,
                    egui::Id::new("example items"),
                    true,
                    title,
                    |ui| {
                        for (app_id, entity_dbs) in example_recordings {
                            dataset_and_its_recordings_ui(
                                ui,
                                ctx,
                                &EntryKind::Local(app_id.clone()),
                                entity_dbs,
                            );
                        }
                    },
                )
                .item_response
        };

        if response.clicked() {
            ctx.command_sender()
                .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(
                    EXAMPLES_ORIGIN.clone(),
                )));
            ctx.command_sender()
                .send_system(SystemCommand::SetSelection(Item::RedapServer(
                    EXAMPLES_ORIGIN.clone(),
                )));
        }
    }
}

fn add_button_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
    use re_ui::list_item::ItemButton as _;
    Box::new(ItemMenuButton::new(&re_ui::icons::ADD, "Add…", |ui| {
        if re_ui::UICommand::Open
            .menu_button_ui(ui, ctx.command_sender())
            .clicked()
        {
            ui.close();
        };
        if re_ui::UICommand::AddRedapServer
            .menu_button_ui(ui, ctx.command_sender())
            .clicked()
        {
            ui.close();
        };
    }))
    .ui(ui)
    .on_hover_text("Open a file or connect to a server");
}
