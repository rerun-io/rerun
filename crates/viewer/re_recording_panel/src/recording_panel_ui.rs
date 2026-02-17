use std::sync::Arc;

use egui::{RichText, Widget as _};
use re_data_ui::DataUi as _;
use re_data_ui::item_ui::{entity_db_button_ui, table_id_button_ui};
use re_log_channel::LogSource;
use re_log_types::TableId;
use re_redap_browser::{Command, EXAMPLES_ORIGIN, RedapServers};
use re_ui::list_item::{LabelContent, ListItemContentButtonsExt as _};
use re_ui::{OnResponseExt as _, UiExt as _, UiLayout, icons, list_item};
use re_viewer_context::open_url::ViewerOpenUrl;
use re_viewer_context::{
    DisplayMode, EditRedapServerModalCommand, Item, RecordingOrTable, SystemCommand,
    SystemCommandSender as _, ViewerContext,
};

use crate::RecordingPanelCommand;
use crate::data::{
    AppIdData, DatasetData, EntryData, FailedEntryData, RecordingPanelData, RemoteTableData,
    SegmentData, ServerData, ServerEntriesData,
};

#[derive(Debug, Clone, Default)]
pub struct RecordingPanel {
    commands: Vec<RecordingPanelCommand>,
}

impl RecordingPanel {
    pub fn send_command(&mut self, command: RecordingPanelCommand) {
        self.commands.push(command);
    }

    pub fn show_panel(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        servers: &RedapServers,
        hide_examples: bool,
    ) {
        re_tracing::profile_function!();
        let recording_panel_data = RecordingPanelData::new(ctx, servers, hide_examples);

        for command in self.commands.drain(..) {
            match command {
                RecordingPanelCommand::SelectNextRecording => {
                    shift_through_recordings(ctx, &recording_panel_data, 1);
                }
                RecordingPanelCommand::SelectPreviousRecording => {
                    shift_through_recordings(ctx, &recording_panel_data, -1);
                }
            }
        }

        ui.panel_content(|ui| {
            ui.panel_title_bar_with_buttons(
                "Sources",
                Some("Your connected servers, opened recordings and tables."),
                |ui| {
                    add_button_ui(ctx, ui, &recording_panel_data);
                },
            );
        });

        egui::ScrollArea::both()
            .id_salt("recordings_scroll_area")
            .auto_shrink([false, false]) // shrinking forces to limit maximum height of the recording panel
            .show(ui, |ui| {
                ui.panel_content(|ui| {
                    re_ui::list_item::list_item_scope(ui, "recording panel", |ui| {
                        all_sections_ui(ctx, ui, servers, &recording_panel_data);
                    });
                });
            });
    }
}

fn shift_through_recordings(
    ctx: &ViewerContext<'_>,
    recording_panel_data: &RecordingPanelData<'_>,
    direction: isize,
) {
    let current_store_id = ctx.store_context.recording.store_id();

    #[expect(clippy::cast_possible_wrap)]
    if let Some((idx, store_collection)) =
        recording_panel_data.collection_from_recording(current_store_id)
    {
        let len = store_collection.len() as isize;
        let new_idx = ((idx as isize + direction + len) % len) as usize;

        // TODO(#11792): this whole feature would be massively more useful if we left the selection
        // alone and tried to maintain viewer state when switching recording (including current
        // timeline, time point, selection, etc.)
        ctx.command_sender()
            .send_system(SystemCommand::SetSelection(
                Item::StoreId(store_collection[new_idx].store_id().clone()).into(),
            ));
    }
}

fn add_button_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    _recording_panel_data: &RecordingPanelData<'_>,
) {
    ui.add(
        ui.small_icon_button_widget(&re_ui::icons::ADD, "Add…")
            .on_hover_text("Open a file or connect to a server")
            .on_menu(|ui| {
                if re_ui::UICommand::Open
                    .menu_button_ui(ui, ctx.command_sender())
                    .clicked()
                {
                    ui.close();
                }
                if re_ui::UICommand::AddRedapServer
                    .menu_button_ui(ui, ctx.command_sender())
                    .clicked()
                {
                    ui.close();
                }

                // Show some nice debugging tools in debug builds.
                #[cfg(debug_assertions)]
                {
                    ui.separator();
                    ui.add_enabled(
                        false,
                        egui::Button::new(egui::RichText::new("Debug-only tools").italics()),
                    );

                    if ui.button("Print recording entity DBs").clicked() {
                        let recording_entity_dbs = ctx
                            .storage_context
                            .bundle
                            .entity_dbs()
                            .filter(|entity_db| entity_db.store_id().is_recording())
                            .collect::<Vec<_>>();
                        println!("Recording entity DBs:\n{recording_entity_dbs:#?}\n");
                    }

                    if ui.button("Print recording panel data").clicked() {
                        println!("Recording panel data:\n{_recording_panel_data:#?}\n");
                    }
                }
            }),
    );
}

fn all_sections_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    servers: &RedapServers,
    recording_panel_data: &RecordingPanelData<'_>,
) {
    //
    // Welcome and examples
    //

    if recording_panel_data.show_example_section {
        welcome_item_ui(ctx, ui, recording_panel_data);
    }

    //
    // Empty placeholder
    //

    if recording_panel_data.is_empty() {
        ui.add_space(ui.tokens().panel_margin().left as f32);
        ui.weak("Click + to add a recording, connect to a server or drag and drop a file directly to the viewer");
    }

    //
    // Servers
    //

    for server_data in &recording_panel_data.servers {
        server_section_ui(ctx, ui, servers, server_data);
    }

    //
    // Local recordings and tables
    //

    if !recording_panel_data.local_apps.is_empty() || !recording_panel_data.local_tables.is_empty()
    {
        let id = egui::Id::new("local items");
        if ui
            .list_item()
            .header()
            .show_hierarchical_with_children(
                ui,
                id,
                true,
                list_item::LabelContent::header("Local"),
                |ui| {
                    for app_id_data in &recording_panel_data.local_apps {
                        app_id_section_ui(ctx, ui, app_id_data);
                    }

                    for table_id in &recording_panel_data.local_tables {
                        table_item_ui(ctx, ui, table_id);
                    }
                },
            )
            .item_response
            .clicked()
        {
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                true,
            );
            state.toggle(ui);
            state.store(ui.ctx());
        }
    }

    //
    // Loading receivers
    //

    loading_receivers_ui(ctx, ui, &recording_panel_data.loading_receivers);

    // Add space at the end of the recordings panel
    ui.add_space(8.0);
}

fn welcome_item_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    recording_panel_data: &RecordingPanelData<'_>,
) {
    let item = Item::RedapServer(EXAMPLES_ORIGIN.clone());
    let selected = ctx.is_selected_or_loading(&item);
    let active = matches!(
        ctx.display_mode(),
        DisplayMode::RedapServer(origin) if origin == &*EXAMPLES_ORIGIN
    );

    let title = list_item::LabelContent::header("Welcome to rerun").with_icon(&icons::HOME);

    let list_item = ui.list_item().header().selected(selected).active(active);

    let response = if recording_panel_data.example_apps.is_empty() {
        list_item.show_flat(ui, title)
    } else {
        list_item
            .show_hierarchical_with_children(
                ui,
                egui::Id::new("example items"),
                true,
                title,
                |ui| {
                    for app_id_data in &recording_panel_data.example_apps {
                        app_id_section_ui(ctx, ui, app_id_data);
                    }
                },
            )
            .item_response
    };

    ctx.handle_select_hover_drag_interactions(&response, item.clone(), false);
    ctx.handle_select_focus_sync(&response, item);

    if response.clicked() {
        re_redap_browser::switch_to_welcome_screen(ctx.command_sender());
    }
}

// ---

fn server_section_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    servers: &RedapServers,
    server_data: &ServerData<'_>,
) {
    let ServerData {
        origin,
        is_active,
        is_selected,
        entries_data,
    } = server_data;

    let content = list_item::LabelContent::header(origin.host.to_string())
        .with_always_show_buttons(true)
        .with_button(
            ui.small_icon_button_widget(&icons::MORE, "Actions")
                .on_menu(move |ui| {
                    if icons::RESET
                        .as_button_with_label(ui.tokens(), "Refresh")
                        .ui(ui)
                        .clicked()
                    {
                        servers.send_command(Command::RefreshCollection(origin.clone()));
                    }
                    if icons::SETTINGS
                        .as_button_with_label(ui.tokens(), "Edit")
                        .ui(ui)
                        .clicked()
                    {
                        servers.send_command(Command::OpenEditServerModal(
                            EditRedapServerModalCommand::new(origin.clone()),
                        ));
                    }
                    if icons::TRASH
                        .as_button_with_label(ui.tokens(), "Remove")
                        .ui(ui)
                        .clicked()
                    {
                        servers.send_command(Command::RemoveServer(origin.clone()));
                    }
                }),
        );

    let item_response = ui
        .list_item()
        .header()
        .selected(*is_selected)
        .active(*is_active)
        .show_hierarchical_with_children(
            ui,
            egui::Id::new(origin).with("server_item"),
            true,
            content,
            |ui| {
                server_entries_ui(ctx, ui, entries_data);
            },
        )
        .item_response
        .on_hover_text(origin.to_string());

    ctx.handle_select_hover_drag_interactions(&item_response, server_data.item(), false);
    ctx.handle_select_focus_sync(&item_response, server_data.item());

    if item_response.clicked() {
        ctx.command_sender()
            .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(
                origin.clone(),
            )));
    }
}

fn server_entries_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    entries_data: &ServerEntriesData<'_>,
) {
    match entries_data {
        ServerEntriesData::Loading => {
            ui.list_item_flat_noninteractive(list_item::CustomContent::new(|ui, _| {
                // TODO(emilk): ideally we should show this loading indicator left of the server name,
                // instead of the expand-chevron.
                ui.loading_indicator();
            }));
        }

        ServerEntriesData::Error {
            message,
            is_auth_error,
        } => {
            let (label, color) = if *is_auth_error {
                ("Authentication required", ui.visuals().weak_text_color())
            } else {
                ("Failed to load entries", ui.visuals().error_fg_color)
            };
            ui.list_item_flat_noninteractive(list_item::LabelContent::new(
                egui::RichText::new(label).color(color),
            ))
            .on_hover_text(message);
        }

        ServerEntriesData::Loaded {
            dataset_entries,
            table_entries,
            failed_entries,
        } => {
            for dataset in dataset_entries {
                dataset_entry_ui(ctx, ui, dataset);
            }

            for table in table_entries {
                remote_table_entry_ui(ctx, ui, table);
            }

            for failed_entry in failed_entries {
                failed_entry_ui(ctx, ui, failed_entry);
            }
        }
    }
}

fn dataset_entry_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    dataset_entry_data: &DatasetData<'_>,
) {
    let DatasetData {
        entry_data:
            EntryData {
                origin,
                entry_id,
                name,
                icon,
                is_selected,
                is_active,
            },
        displayed_segments,
    } = dataset_entry_data;

    let item = dataset_entry_data.entry_data.item();
    let list_item = ui.list_item().selected(*is_selected).active(*is_active);

    let mut list_item_content = re_ui::list_item::LabelContent::new(name).with_icon(icon);

    let id = ui.make_persistent_id(dataset_entry_data.entry_data.id());

    if !displayed_segments.is_empty() {
        list_item_content = list_item_content.with_buttons(|ui| {
            // Close-button:
            let resp = ui
                .small_icon_button(&icons::CLOSE_SMALL, "Close all recordings in this dataset")
                .on_hover_text("Close all recordings in this dataset. This cannot be undone.");

            if resp.clicked() {
                for db in displayed_segments.iter().filter_map(SegmentData::entity_db) {
                    ctx.command_sender()
                        .send_system(SystemCommand::CloseRecordingOrTable(
                            RecordingOrTable::Recording {
                                store_id: db.store_id().clone(),
                            },
                        ));
                }
            }
        });
    }

    let item_response = if !displayed_segments.is_empty() {
        list_item
            .show_hierarchical_with_children(ui, id, true, list_item_content, |ui| {
                for segment in displayed_segments {
                    match segment {
                        SegmentData::Loading { receiver } => receiver_ui(ctx, ui, receiver, true),

                        SegmentData::Loaded { entity_db } => {
                            let include_app_id = false; // we already show it in the parent item
                            let response = entity_db_button_ui(
                                ctx,
                                ui,
                                entity_db,
                                UiLayout::SelectionPanel,
                                include_app_id,
                            );
                            ctx.handle_select_focus_sync(
                                &response,
                                Item::StoreId(entity_db.store_id().clone()),
                            );
                        }
                    }
                }
            })
            .item_response
    } else {
        list_item.show_hierarchical(ui, list_item_content)
    };

    let item_response = item_response.on_hover_ui(|ui| {
        ui.label(format!("Dataset: {name:?}"));
    });

    let new_display_mode =
        DisplayMode::RedapEntry(re_uri::EntryUri::new(origin.clone(), *entry_id));

    item_response.context_menu(|ui| {
        let url = ViewerOpenUrl::from_display_mode(ctx.storage_context.hub, &new_display_mode)
            .and_then(|url| url.sharable_url(None));
        if ui
            .add_enabled(url.is_ok(), egui::Button::new("Copy link to dataset"))
            .on_disabled_hover_text("Can't copy a link to this dataset")
            .clicked()
            && let Ok(url) = url
        {
            ctx.command_sender()
                .send_system(SystemCommand::CopyViewerUrl(url));
        }

        if ui.button("Copy dataset name").clicked() {
            re_log::info!("Copied {name:?} to clipboard");
            ui.ctx().copy_text(name.clone());
        }

        if ui.button("Copy dataset id").clicked() {
            let id = entry_id.id.to_string();
            re_log::info!("Copied {id:?} to clipboard");
            ui.ctx().copy_text(id);
        }
    });

    ctx.handle_select_hover_drag_interactions(&item_response, item.clone(), false);
    ctx.handle_select_focus_sync(&item_response, item.clone());

    if item_response.clicked() {
        ctx.command_sender()
            .send_system(SystemCommand::set_selection(item));
        ctx.command_sender()
            .send_system(SystemCommand::ChangeDisplayMode(new_display_mode));
    }
}

fn remote_table_entry_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    remote_table_data: &RemoteTableData,
) {
    let RemoteTableData {
        entry_data:
            EntryData {
                origin,
                entry_id,
                name,
                icon,
                is_selected,
                is_active,
            },
    } = remote_table_data;

    let item = remote_table_data.entry_data.item();
    let text = RichText::new(name);

    let list_item = ui.list_item().selected(*is_selected).active(*is_active);
    let list_item_content = LabelContent::new(text).with_icon(icon);
    let item_response = list_item.show_hierarchical(ui, list_item_content);

    ctx.handle_select_hover_drag_interactions(&item_response, item.clone(), false);
    ctx.handle_select_focus_sync(&item_response, item.clone());

    if item_response.clicked() {
        ctx.command_sender()
            .send_system(SystemCommand::set_selection(item));
        ctx.command_sender()
            .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapEntry(
                re_uri::EntryUri::new(origin.clone(), *entry_id),
            )));
    }
}

fn failed_entry_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    failed_entry_data: &FailedEntryData,
) {
    let FailedEntryData {
        entry_data:
            EntryData {
                origin,
                entry_id,
                name,
                icon,
                is_selected,
                is_active,
            },
        error,
    } = failed_entry_data;

    let item = failed_entry_data.entry_data.item();
    let text = RichText::new(name).color(ui.visuals().error_fg_color);

    let list_item = ui.list_item().selected(*is_selected).active(*is_active);
    let list_item_content = LabelContent::new(text).with_icon(icon);
    let item_response = list_item.show_hierarchical(ui, list_item_content);

    if item_response.clicked() {
        ctx.command_sender()
            .send_system(SystemCommand::set_selection(item));
        ctx.command_sender()
            .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapEntry(
                re_uri::EntryUri::new(origin.clone(), *entry_id),
            )));
    }

    item_response.on_hover_text(error);
}

// ---

fn app_id_section_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, local_app_id: &AppIdData<'_>) {
    let AppIdData {
        app_id,
        is_active,
        is_selected,
        loaded_recordings,
    } = local_app_id;

    let item = local_app_id.item();
    let list_item = ui.list_item().selected(*is_selected).active(*is_active);

    let mut list_item_content =
        re_ui::list_item::LabelContent::new(local_app_id.name()).with_icon(&icons::DATASET);

    let id = ui.make_persistent_id(local_app_id.id());

    if !local_app_id.loaded_recordings.is_empty() {
        list_item_content = list_item_content.with_buttons(|ui| {
            // Close-button:
            let resp = ui
                .small_icon_button(&icons::CLOSE_SMALL, "Close all recordings in this dataset")
                .on_hover_text("Close all recordings in this dataset. This cannot be undone.");

            if resp.clicked() {
                ctx.command_sender()
                    .send_system(SystemCommand::CloseApp(app_id.clone()));
            }
        });
    }

    let mut item_response = if !loaded_recordings.is_empty() {
        list_item
            .show_hierarchical_with_children(ui, id, true, list_item_content, |ui| {
                for recording_data in loaded_recordings {
                    let include_app_id = false; // we already show it in the parent item
                    let response = entity_db_button_ui(
                        ctx,
                        ui,
                        recording_data.entity_db,
                        UiLayout::SelectionPanel,
                        include_app_id,
                    );
                    ctx.handle_select_focus_sync(
                        &response,
                        Item::StoreId(recording_data.entity_db.store_id().clone()),
                    );
                }
            })
            .item_response
    } else {
        list_item.show_hierarchical(ui, list_item_content)
    };

    item_response = item_response.on_hover_ui(|ui| {
        app_id.data_ui_recording(ctx, ui, UiLayout::Tooltip);
    });

    ctx.handle_select_hover_drag_interactions(&item_response, item.clone(), false);
    ctx.handle_select_focus_sync(&item_response, item);
    if list_item::ListItem::gained_focus_via_arrow_key(ui.ctx(), item_response.id) {
        ctx.command_sender()
            .send_system(SystemCommand::ActivateApp(app_id.clone()));
    }

    if item_response.clicked() {
        //TODO(ab): shouldn't this be done by handle_select_hover_drag_interactions?
        ctx.command_sender()
            .send_system(SystemCommand::ActivateApp(app_id.clone()));
    }
}

fn table_item_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, table_id: &TableId) {
    table_id_button_ui(ctx, ui, table_id, UiLayout::SelectionPanel);
}

fn loading_receivers_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    loading_receivers: &Vec<Arc<LogSource>>,
) {
    for receiver in loading_receivers {
        receiver_ui(ctx, ui, receiver, false);
    }
}

fn receiver_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    receiver: &LogSource,
    show_hierarchal: bool,
) {
    let Some(name) = receiver.loading_name() else {
        return;
    };

    let selected = ctx.is_selected_or_loading(&Item::DataSource(receiver.clone()));

    let label_content = re_ui::list_item::LabelContent::new(&name)
        .with_icon_fn(|ui, rect, _| {
            re_ui::loading_indicator::paint_loading_indicator_inside(
                ui,
                egui::Align2::CENTER_CENTER,
                rect,
                1.0,
            );
        })
        .with_buttons(|ui| {
            let resp = ui
                .small_icon_button(&re_ui::icons::REMOVE, "Disconnect")
                .on_hover_text("Disconnect from this source");

            if resp.clicked() {
                ctx.connected_receivers.remove(receiver);
            }
        });

    let response = if show_hierarchal {
        ui.list_item()
            .selected(selected)
            .show_hierarchical(ui, label_content)
    } else {
        ui.list_item()
            .selected(selected)
            .show_flat(ui, label_content)
    };

    response.context_menu(|ui| {
        let url = ViewerOpenUrl::from_data_source(receiver).and_then(|url| url.sharable_url(None));
        if ui
            .add_enabled(url.is_ok(), egui::Button::new("Copy link to segment"))
            .on_disabled_hover_text(if let Err(err) = url.as_ref() {
                format!("Can't copy a link to this segment: {err}")
            } else {
                "Can't copy a link to this segment".to_owned()
            })
            .clicked()
            && let Ok(url) = url
        {
            ctx.command_sender()
                .send_system(SystemCommand::CopyViewerUrl(url));
        }

        if ui.button("Copy segment name").clicked() {
            re_log::info!("Copied {name:?} to clipboard");
            ui.ctx().copy_text(name);
        }
    });
}
