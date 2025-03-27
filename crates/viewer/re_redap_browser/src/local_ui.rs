use crate::context::Context;
use crate::servers::Command;
use egui::{Id, Ui};
use re_log_types::{ApplicationId, StoreKind};
use re_smart_channel::SmartChannelSource;
use re_ui::{icons, list_item, UiExt};
use re_viewer_context::external::re_entity_db::EntityDb;
use re_viewer_context::ViewerContext;
use std::collections::BTreeMap;

pub fn local_ui(ui: &mut Ui, viewer_ctx: &ViewerContext, ctx: &Context) {
    let datasets = sort_datasets(viewer_ctx);

    if !datasets.local_recordings.is_empty() {
        ui.list_item()
            .header()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                Id::new("local_datasets"),
                true,
                list_item::LabelContent::header("Local storage"),
                |ui| {
                    for (id, entities) in datasets.local_recordings {
                        local_dataset_ui(ui, &id, entities);
                    }
                },
            );
    }

    let label = list_item::LabelContent::header("Rerun examples");
    let response = if datasets.example_recordings.is_empty() {
        ui.list_item().show_flat(ui, label)
    } else {
        ui.list_item()
            .header()
            .show_hierarchical_with_children(ui, Id::new("example_datasets"), true, label, |ui| {
                for (id, entities) in datasets.example_recordings {
                    local_dataset_ui(ui, &id, entities);
                }
            })
            .item_response
    };

    if response.clicked() {
        ctx.command_sender
            .send(Command::SelectServer(re_uri::Origin::examples_origin()))
            .ok();
    }
}

fn local_dataset_ui(ui: &mut Ui, app_id: &ApplicationId, entities: Vec<&EntityDb>) {
    ui.list_item().show_flat(
        ui,
        list_item::LabelContent::new(app_id.as_str()).with_icon(&icons::DATASET),
    );
}

pub struct SortDatasetsResults<'a> {
    // TODO(lucasmerlin): Replace String with DatasetId or whatever we come up with
    pub remote_recordings: BTreeMap<re_uri::Origin, BTreeMap<String, Vec<&'a EntityDb>>>,
    pub example_recordings: BTreeMap<ApplicationId, Vec<&'a EntityDb>>,
    pub local_recordings: BTreeMap<ApplicationId, Vec<&'a EntityDb>>,
}
pub fn sort_datasets<'a>(viewer_ctx: &ViewerContext<'a>) -> SortDatasetsResults<'a> {
    let mut remote_recordings: BTreeMap<re_uri::Origin, BTreeMap<String, Vec<&EntityDb>>> =
        BTreeMap::new();
    let mut local_recordings: BTreeMap<ApplicationId, Vec<&EntityDb>> = BTreeMap::new();
    let mut example_recordings: BTreeMap<ApplicationId, Vec<&EntityDb>> = BTreeMap::new();

    for entity_db in viewer_ctx.store_context.bundle.entity_dbs() {
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
                .entry("default".to_owned())
                .or_default();

            if entity_db.store_kind() == StoreKind::Recording {
                dataset_recordings.push(entity_db);
            }
        } else if entity_db.store_kind() == StoreKind::Recording {
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

    SortDatasetsResults {
        remote_recordings,
        example_recordings,
        local_recordings,
    }
}
