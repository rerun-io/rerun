//! Implement a global drag-and-drop payload type that enable dragging from various parts of the UI
//! (e.g., from the streams tree to the viewport, etc.).

use std::fmt::Formatter;

use itertools::Itertools;

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

    /// The dragged content is made of a collection of [`Item`]s we do know how to handle.
    Invalid,
}

impl DragAndDropPayload {
    pub fn from_items(selected_items: &ItemCollection) -> Self {
        if let Ok(contents) = selected_items.try_into() {
            Self::Contents { contents }
        } else {
            Self::Invalid
        }
    }
}

impl std::fmt::Display for DragAndDropPayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contents { contents } => items_to_string(
                contents
                    .iter()
                    .map(|content| content.as_item())
                    .collect_vec()
                    .iter(),
            )
            .fmt(f),

            // this is not used in the UI
            Self::Invalid => "invalid selection".fmt(f),
        }
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

            let response = drag_pill_frame(matches!(
                payload.as_ref(),
                &DragAndDropPayload::Invalid { .. }
            ))
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

fn drag_pill_frame(error_state: bool) -> egui::Frame {
    let hue = if error_state { Hue::Red } else { Hue::Blue };

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

fn items_to_string<'a>(items: impl Iterator<Item = &'a Item>) -> String {
    let mut container_cnt = 0u32;
    let mut view_cnt = 0u32;
    let mut app_cnt = 0u32;
    let mut data_source_cnt = 0u32;
    let mut store_cnt = 0u32;
    let mut entity_cnt = 0u32;
    let mut instance_cnt = 0u32;
    let mut component_cnt = 0u32;

    for item in items {
        match item {
            Item::Container(_) => container_cnt += 1,
            Item::View(_) => view_cnt += 1,
            Item::AppId(_) => app_cnt += 1,
            Item::DataSource(_) => data_source_cnt += 1,
            Item::StoreId(_) => store_cnt += 1,
            Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => {
                if instance_path.is_all() {
                    entity_cnt += 1;
                } else {
                    instance_cnt += 1;
                }
            }
            Item::ComponentPath(_) => component_cnt += 1,
        }
    }

    let count_and_names = [
        (container_cnt, "container", "containers"),
        (view_cnt, "view", "views"),
        (app_cnt, "app", "apps"),
        (data_source_cnt, "data source", "data sources"),
        (store_cnt, "store", "stores"),
        (entity_cnt, "entity", "entities"),
        (instance_cnt, "instance", "instances"),
        (component_cnt, "component", "components"),
    ];

    count_and_names
        .into_iter()
        .filter_map(|(count, name_singular, name_plural)| {
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
}
