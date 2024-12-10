//! Implement a global drag-and-drop payload type that enable dragging from various parts of the UI
//! (e.g., from the streams tree to the viewport, etc.).

use std::fmt::{Display, Formatter};

use itertools::Itertools;
use re_entity_db::InstancePath;
use re_log_types::EntityPath;
use re_ui::{
    ColorToken, Hue,
    Scale::{S325, S375},
    UiExt,
};

use crate::{Contents, Item, ItemCollection};

//TODO(ab): add more type of things we can drag, in particular entity paths
#[derive(Debug)]
pub enum DragAndDropPayload {
    /// The dragged content is made only of [`Contents`].
    Contents { contents: Vec<Contents> },

    /// The dragged content is made of entities.
    Entities { entities: Vec<EntityPath> },

    /// The dragged content is made of a collection of [`Item`]s we do know how to handle.
    Invalid,
}

impl DragAndDropPayload {
    pub fn from_items(selected_items: &ItemCollection) -> Self {
        if let Some(contents) = try_item_collection_to_contents(selected_items) {
            Self::Contents { contents }
        } else if let Some(entities) = try_item_collection_to_entities(selected_items) {
            Self::Entities { entities }
        } else {
            Self::Invalid
        }
    }
}

fn try_item_collection_to_contents(items: &ItemCollection) -> Option<Vec<Contents>> {
    items.iter().map(|(item, _)| item.try_into().ok()).collect()
}

fn try_item_collection_to_entities(items: &ItemCollection) -> Option<Vec<EntityPath>> {
    items
        .iter()
        .map(|(item, _)| match item {
            Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => instance_path
                .is_all()
                .then(|| instance_path.entity_path.clone()),
            _ => None,
        })
        .collect()
}

impl std::fmt::Display for DragAndDropPayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut item_counter = ItemCounter::default();

        match self {
            Self::Contents { contents } => {
                for content in contents {
                    item_counter.add(&content.as_item());
                }
            }

            Self::Entities { entities } => {
                for entity in entities {
                    item_counter.add(&Item::InstancePath(InstancePath::from(entity.clone())));
                }
            }

            // this is not used in the UI
            Self::Invalid => {}
        }

        item_counter.fmt(f)
    }
}

/// Display the currently dragged payload as a pill in the UI.
///
/// This should be called once per frame.
pub fn drag_and_drop_payload_cursor_ui(ctx: &egui::Context) {
    if let Some(payload) = egui::DragAndDrop::payload::<DragAndDropPayload>(ctx) {
        if let Some(pointer_pos) = ctx.pointer_interact_pos() {
            let icon = match payload.as_ref() {
                DragAndDropPayload::Contents { .. } => &re_ui::icons::DND_MOVE,
                DragAndDropPayload::Entities { .. } => &re_ui::icons::DND_ADD_TO_EXISTING,
                // don't draw anything for invalid selection
                DragAndDropPayload::Invalid => return,
            };

            let layer_id = egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("drag_and_drop_payload_layer"),
            );

            let mut ui = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("rerun_drag_and_drop_payload_ui"),
                egui::UiBuilder::new().layer_id(layer_id),
            );

            ui.set_opacity(0.7);

            let response = drag_pill_frame()
                .show(&mut ui, |ui| {
                    let text_color = ui.visuals().widgets.inactive.text_color();

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;

                        ui.small_icon(icon, Some(text_color));
                        ui.label(egui::RichText::new(payload.to_string()).color(text_color));
                    });
                })
                .response;

            let delta = pointer_pos - response.rect.right_bottom();
            ctx.transform_layer_shapes(layer_id, emath::TSTransform::from_translation(delta));
        }
    }
}

fn drag_pill_frame() -> egui::Frame {
    let hue = Hue::Blue;

    egui::Frame {
        fill: re_ui::design_tokens().color(ColorToken::new(hue, S325)),
        stroke: egui::Stroke::new(
            1.0,
            re_ui::design_tokens().color(ColorToken::new(hue, S375)),
        ),
        rounding: (2.0).into(),
        inner_margin: egui::Margin {
            left: 6.0,
            right: 9.0,
            top: 5.0,
            bottom: 4.0,
        },
        ..Default::default()
    }
}
/// Helper class to count item types and display them in a human-readable way.
#[derive(Debug, Default)]
struct ItemCounter {
    container_cnt: u32,
    view_cnt: u32,
    app_cnt: u32,
    data_source_cnt: u32,
    store_cnt: u32,
    entity_cnt: u32,
    instance_cnt: u32,
    component_cnt: u32,
}

impl ItemCounter {
    fn add(&mut self, item: &Item) {
        match item {
            Item::Container(_) => self.container_cnt += 1,
            Item::View(_) => self.view_cnt += 1,
            Item::AppId(_) => self.app_cnt += 1,
            Item::DataSource(_) => self.data_source_cnt += 1,
            Item::StoreId(_) => self.store_cnt += 1,
            Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => {
                if instance_path.is_all() {
                    self.entity_cnt += 1;
                } else {
                    self.instance_cnt += 1;
                }
            }
            Item::ComponentPath(_) => self.component_cnt += 1,
        }
    }
}

impl Display for ItemCounter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let count_and_names = [
            (&self.container_cnt, "container", "containers"),
            (&self.view_cnt, "view", "views"),
            (&self.app_cnt, "app", "apps"),
            (&self.data_source_cnt, "data source", "data sources"),
            (&self.store_cnt, "store", "stores"),
            (&self.entity_cnt, "entity", "entities"),
            (&self.instance_cnt, "instance", "instances"),
            (&self.component_cnt, "component", "components"),
        ];

        count_and_names
            .into_iter()
            .filter_map(|(&count, name_singular, name_plural)| {
                if count > 0 {
                    Some(format!(
                        "{} {}",
                        re_format::format_uint(count),
                        if count == 1 {
                            name_singular
                        } else {
                            name_plural
                        },
                    ))
                } else {
                    None
                }
            })
            .join(", ")
            .fmt(f)
    }
}
