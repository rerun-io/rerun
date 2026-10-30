use std::collections::BTreeMap;

use egui::{Frame, Margin};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::EntryKind;
use re_ui::UiExt as _;
use re_ui::egui_ext::card_layout::CardLayout;
use re_ui::icons;
use re_uri::{DATASET_HIERARCHY_SEPARATOR, split_dataset_hierarchy_path};
use re_viewer_context::{Route, SystemCommand, SystemCommandSender as _};

use crate::entries::Entry;

enum FolderChildCard {
    Subfolder {
        name: String,
        path_prefix: String,
        entry_count: usize,
    },
    Entry {
        name: String,
        entry_id: EntryId,
        kind: EntryKind,
    },
}

pub fn folder_cards_ui(
    ui: &mut egui::Ui,
    origin: &re_uri::Origin,
    entries: &ahash::HashMap<EntryId, Entry>,
    path_prefix: &str,
    command_sender: &re_viewer_context::CommandSender,
) {
    let children = collect_cards(entries, path_prefix);

    if children.is_empty() {
        ui.label("This folder is empty.");
        return;
    }

    let tokens = ui.tokens();
    let card_min_width = tokens.table_grid_view_card_min_width;
    let card_spacing = tokens.table_grid_view_card_spacing;

    let inner_margin = Margin::same(tokens.table_grid_view_card_inner_margin as i8);
    let card_frame = Frame::new()
        .inner_margin(inner_margin)
        .fill(tokens.card_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(tokens.table_grid_view_card_corner_radius);

    let clicked = egui::ScrollArea::vertical()
        .auto_shrink(false)
        .content_margin(egui::Margin::same(card_spacing as i8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(card_spacing, card_spacing);

            CardLayout::uniform(children.len(), card_min_width + card_spacing, card_frame)
                .all_rows_use_available_width(false)
                .hover_fill(tokens.card_hover_fill)
                .hover_stroke(tokens.card_hover_stroke)
                .clickable()
                .show(ui, |ui, index, _hovered| {
                    let Some(child) = children.get(index) else {
                        return;
                    };

                    match child {
                        FolderChildCard::Subfolder {
                            name, entry_count, ..
                        } => {
                            card_title_ui(ui, &icons::FOLDER, "Folder", name);
                            let label = if *entry_count == 1 {
                                "1 entry".to_owned()
                            } else {
                                format!("{entry_count} entries")
                            };
                            ui.label(label);
                        }
                        FolderChildCard::Entry { name, kind, .. } => {
                            let (icon, tooltip) = match kind {
                                EntryKind::Dataset
                                | EntryKind::DatasetView
                                | EntryKind::BlueprintDataset
                                | EntryKind::AssetDataset => (&icons::DATASET, "Dataset"),
                                EntryKind::Table | EntryKind::TableView => (&icons::TABLE, "Table"),
                                EntryKind::Unspecified => (&icons::VIEW_UNKNOWN, "Entry"),
                            };
                            card_title_ui(ui, icon, tooltip, name);
                        }
                    }
                })
        })
        .inner;

    if let Some(child) = clicked.and_then(|index| children.get(index)) {
        let route = match child {
            FolderChildCard::Subfolder { path_prefix, .. } => Route::RedapFolder {
                origin: origin.clone(),
                path: path_prefix.clone(),
            },
            FolderChildCard::Entry { entry_id, kind, .. } => Route::RedapEntry {
                origin: origin.clone(),
                entry_id: *entry_id,
                kind: crate::entries::route_entry_kind(*kind),
            },
        };

        if let Some(item) = route.item() {
            command_sender.send_system(SystemCommand::set_selection(item));
        }
        command_sender.send_system(SystemCommand::SetRoute(route));
    }
}

fn card_title_ui(ui: &mut egui::Ui, icon: &icons::Icon, kind_name: &str, name: &str) {
    ui.horizontal(|ui| {
        ui.add(
            icon.as_image()
                .fit_to_exact_size(ui.tokens().small_icon_size)
                .tint(ui.visuals().text_color()),
        )
        .on_hover_text(kind_name);
        ui.strong(name);
    });
}

fn collect_cards(
    entries: &ahash::HashMap<EntryId, Entry>,
    path_prefix: &str,
) -> Vec<FolderChildCard> {
    let prefix_with_dot = format!("{path_prefix}{DATASET_HIERARCHY_SEPARATOR}");
    let mut subfolders = BTreeMap::new();
    let mut direct_entries = Vec::new();

    let mut sorted_entries: Vec<_> = entries.values().collect();
    sorted_entries.sort_by_key(|e| e.name().as_str());

    for entry in sorted_entries {
        let name = entry.name().as_str();
        if let Some(rest) = name.strip_prefix(&prefix_with_dot) {
            if rest.is_empty() {
                continue;
            }

            let mut segments = split_dataset_hierarchy_path(rest);
            let Some(subfolder_segment) = segments.next() else {
                continue;
            };

            if segments.next().is_some() {
                // Belongs to a subfolder.
                let card = subfolders
                    .entry(subfolder_segment.to_owned())
                    .or_insert_with(|| FolderChildCard::Subfolder {
                        name: subfolder_segment.to_owned(),
                        path_prefix: format!(
                            "{path_prefix}{DATASET_HIERARCHY_SEPARATOR}{subfolder_segment}"
                        ),
                        entry_count: 0,
                    });
                if let FolderChildCard::Subfolder { entry_count, .. } = card {
                    *entry_count += 1;
                }
            } else {
                // Direct child entry.
                direct_entries.push(FolderChildCard::Entry {
                    name: subfolder_segment.to_owned(),
                    entry_id: entry.id(),
                    kind: entry.details().kind,
                });
            }
        }
    }

    std::iter::chain(subfolders.into_values(), direct_entries).collect()
}
