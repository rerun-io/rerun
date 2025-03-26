use std::collections::BTreeMap;

use re_data_ui::{item_ui::entity_db_button_ui, DataUi as _};
use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, LogMsg, StoreKind};
use re_smart_channel::{ReceiveSet, SmartChannelSource};
use re_types::components::Timestamp;
use re_ui::{icons, UiExt as _};
use re_viewer_context::{
    DisplayMode, Item, StoreHub, SystemCommand, SystemCommandSender as _, UiLayout, ViewerContext,
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
            SmartChannelSource::RrdHttpStream { url, .. } => format!("Loading {url}…"),
            SmartChannelSource::RedapGrpcStream(endpoint) => format!("Loading {endpoint}…"),

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
    // TODO(lucasmerlin): Replace String with DatasetId or whatever we come up with
    let mut remote_recordings: BTreeMap<re_uri::Origin, BTreeMap<String, Vec<&EntityDb>>> =
        BTreeMap::new();
    let mut local_recordings: BTreeMap<ApplicationId, Vec<&EntityDb>> = BTreeMap::new();
    let mut example_recordings: BTreeMap<ApplicationId, Vec<&EntityDb>> = BTreeMap::new();

    for entity_db in ctx.store_context.bundle.entity_dbs() {
        // We want to show all open applications, even if they have no recordings
        let Some(app_id) = entity_db.app_id().cloned() else {
            continue; // this only happens if we haven't even started loading it, or if something is really wrong with it.
        };
        if let Some(SmartChannelSource::RedapGrpcStream(endpoint)) = &entity_db.data_source {
            let origin_recordings = remote_recordings
                .entry(endpoint.origin.clone())
                .or_default();

            let dataset_recordings = origin_recordings
                // Currently a origin only has a single dataset, this should change soon
                .entry("default".to_string())
                .or_default();

            if entity_db.store_kind() == StoreKind::Recording {
                dataset_recordings.push(entity_db);
            }
        } else {
            if entity_db.store_kind() == StoreKind::Recording {
                if matches!(&entity_db.data_source, Some(SmartChannelSource::RrdHttpStream {url, ..}) if url.starts_with("https://app.rerun.io"))
                {
                    let recordings = example_recordings.entry(app_id).or_default();
                    recordings.push(entity_db);
                } else {
                    let recordings = local_recordings.entry(app_id).or_default();
                    recordings.push(entity_db);
                }
            }
        }
    }

    if local_recordings.is_empty() && welcome_screen_state.hide {
        ui.list_item().interactive(false).show_flat(
            ui,
            re_ui::list_item::LabelContent::new("No recordings loaded")
                .weak(true)
                .italics(true),
        );
    }

    let title = |title| egui::RichText::new(title).size(11.0).strong();

    for (origin, dataset_recordings) in remote_recordings {
        ui.list_item().show_hierarchical_with_children(
            ui,
            egui::Id::new(&origin),
            true,
            re_ui::list_item::LabelContent::new(title(origin.host.to_string())),
            |ui| {
                for (dataset, entity_dbs) in dataset_recordings {
                    dataset_and_its_recordings_ui(
                        ctx,
                        ui,
                        DatasetKind::Remote(origin.clone(), dataset.clone()),
                        entity_dbs,
                    );
                }
            },
        );
    }

    if !local_recordings.is_empty() {
        ui.list_item().show_hierarchical_with_children(
            ui,
            egui::Id::new("local items"),
            true,
            re_ui::list_item::LabelContent::new(title("Local recordings".to_owned())),
            |ui| {
                for (app_id, entity_dbs) in local_recordings {
                    dataset_and_its_recordings_ui(
                        ctx,
                        ui,
                        DatasetKind::Local(app_id.clone()),
                        entity_dbs,
                    );
                }
            },
        );
    }

    // Always show welcome screen last, if at all:
    if ctx
        .app_options()
        .include_welcome_screen_button_in_recordings_panel
        && !welcome_screen_state.hide
        && !example_recordings.is_empty()
    {
        let response = ui.list_item().show_hierarchical_with_children(
            ui,
            egui::Id::new("example items"),
            true,
            re_ui::list_item::LabelContent::new(title("Rerun examples".to_owned())),
            |ui| {
                for (app_id, entity_dbs) in example_recordings {
                    dataset_and_its_recordings_ui(
                        ctx,
                        ui,
                        DatasetKind::Local(app_id.clone()),
                        entity_dbs,
                    );
                }
            },
        );

        if response.item_response.clicked() {
            DatasetKind::Local(StoreHub::welcome_screen_app_id()).select(ctx);
        }
    }
}

#[derive(Clone, Hash)]
enum DatasetKind {
    Remote(re_uri::Origin, String),
    Local(ApplicationId),
}

impl DatasetKind {
    fn name(&self) -> &str {
        match self {
            DatasetKind::Remote(_, dataset) => dataset,
            DatasetKind::Local(app_id) => app_id.as_str(),
        }
    }

    fn select(&self, ctx: &ViewerContext<'_>) {
        match self {
            DatasetKind::Remote(origin, dataset) => {
                ctx.command_sender()
                    .send_system(SystemCommand::SelectRedapDataset {
                        origin: origin.clone(),
                        dataset: dataset.clone(),
                    });
                ctx.command_sender()
                    .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapBrowser));
            }
            DatasetKind::Local(app) => {
                ctx.command_sender()
                    .send_system(re_viewer_context::SystemCommand::ActivateApp(app.clone()));
                ctx.command_sender()
                    .send_system(SystemCommand::SetSelection(Item::AppId(app.clone())))
            }
        }
    }

    fn item(&self) -> Option<Item> {
        match self {
            DatasetKind::Remote(_, _) => None,
            DatasetKind::Local(app_id) => Some(Item::AppId(app_id.clone())),
        }
    }

    fn is_active(&self, ctx: &ViewerContext<'_>) -> bool {
        match self {
            DatasetKind::Remote(origin, _dataset) => ctx
                .store_context
                .recording
                .data_source
                .as_ref()
                .is_some_and(|source| match source {
                    SmartChannelSource::RedapGrpcStream(endpoint) => {
                        &endpoint.origin == origin // TODO(lucasmerlin): Also check for dataset
                    }
                    _ => false,
                }),
            DatasetKind::Local(app_id) => &ctx.store_context.app_id == app_id,
        }
    }

    fn close(&self, ctx: &ViewerContext<'_>, dbs: &Vec<&EntityDb>) {
        match self {
            DatasetKind::Remote(origin, dataset) => {
                for db in dbs {
                    ctx.command_sender()
                        .send_system(SystemCommand::CloseStore(db.store_id()));
                }
            }
            DatasetKind::Local(app_id) => {
                ctx.command_sender()
                    .send_system(SystemCommand::CloseApp(app_id.clone()));
            }
        }
    }
}

fn dataset_and_its_recordings_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    kind: DatasetKind,
    mut entity_dbs: Vec<&EntityDb>,
) {
    entity_dbs.sort_by_key(|entity_db| entity_db.recording_property::<Timestamp>());

    let selected = kind
        .item()
        .is_some_and(|i| ctx.selection().contains_item(&i));

    let app_list_item = ui.list_item().selected(selected);
    let app_list_item_content =
        re_ui::list_item::LabelContent::new(kind.name()).with_icon_fn(|ui, rect, visuals| {
            // Color icon based on whether this is the active dataset or not:
            let color = if kind.is_active(ctx) {
                visuals.fg_stroke.color
            } else {
                ui.visuals().widgets.noninteractive.fg_stroke.color
            };
            icons::DATASET.as_image().tint(color).paint_at(ui, rect);
        });

    let mut item_response = if matches!(&kind, DatasetKind::Local(id) if id == &StoreHub::welcome_screen_app_id())
    {
        // Special case: the welcome screen never has any recordings
        debug_assert!(
            entity_dbs.is_empty(),
            "There shouldn't be any recording for the welcome screen, but there are!"
        );
        app_list_item.show_hierarchical(ui, app_list_item_content)
    } else {
        // Normal application
        let id = ui.make_persistent_id(&kind);
        let app_list_item_content = app_list_item_content.with_buttons(|ui| {
            // Close-button:
            let resp = ui
                .small_icon_button(&icons::REMOVE)
                .on_hover_text("Close this dataset and all its recordings. This cannot be undone.");
            if resp.clicked() {
                kind.close(ctx, &entity_dbs);
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
                    for entity_db in &entity_dbs {
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

    match &kind {
        DatasetKind::Local(app) => {
            item_response = item_response.on_hover_ui(|ui| {
                app.data_ui_recording(ctx, ui, UiLayout::Tooltip);
            });

            ctx.handle_select_hover_drag_interactions(
                &item_response,
                Item::AppId(app.clone()),
                false,
            );
        }
        _ => {}
    }

    if item_response.clicked() {
        kind.select(ctx);
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
