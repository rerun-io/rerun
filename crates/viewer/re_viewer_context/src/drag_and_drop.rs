//! Support for viewer-wide drag-and-drop of [`crate::Item`]s.
//!
//! ## Theory of operation
//!
//! ### Setup
//!
//! A [`DragAndDropManager`] should be created at the start of the frame and made available to the
//! entire UI code.
//!
//!
//! ### Initiating a drag
//!
//! Any UI representation of an [`crate::Item`] may initiate a drag.
//! [`crate::ViewerContext::handle_select_hover_drag_interactions`] will handle that automatically
//! when passed `true` for its `draggable` argument.
//!
//!
//! ### Reacting to a drag and accepting a drop
//!
//! This part of the process is more involved and typically includes the following steps:
//!
//! 1. When hovered, the receiving UI element should check for a compatible payload using
//!    [`egui::DragAndDrop::payload`] and matching one or more variants of the returned
//!    [`DragAndDropPayload`], if any.
//!
//! 2. If an acceptable payload type is being dragged, the UI element should provide appropriate
//!    visual feedback. This includes:
//!    - Calling [`DragAndDropManager::set_feedback`] with the appropriate feedback.
//!    - Drawing a frame around the target container with
//!      [`re_ui::DesignTokens::drop_target_container_stroke`].
//!    - Optionally provide more feedback, e.g., where exactly the payload will be inserted within
//!      the container.
//!
//! 3. If the mouse is released (using [`egui::PointerState::any_released`]), the payload must be
//!    actually transferred to the container and [`egui::DragAndDrop::clear_payload`] must be
//!    called.

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
        // Note: this is not a filter map, because we rely on the implicit "all" semantics of
        // `collect`: we return `Some<Vec<_>>` only if all iterated items are `Some<_>`.
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragAndDropFeedback {
    /// The payload type is irrelevant to me.
    ///
    /// For example, dropping a view and/or contain onto an existing view in the viewport is
    /// irrelevant.
    ///
    /// This is the default displayed feedback, unless explicitly set otherwise by some UI hovered
    /// UI element.
    #[default]
    Ignore,

    /// The payload type is acceptable and could successfully be dropped at the current location.
    Accept,

    /// The payload type is correct, but it's content cannot be accepted by the current drop location.
    ///
    /// For example, a view might reject an entity because it already contains it.
    Reject,
}

/// Helper to handle drag-and-drop operations.
///
/// This helper must be constructed at the beginning of the frame and disposed of at the end.
/// Its [`Self::payload_cursor_ui`] method should be called late during the frame (after the rest of
/// the UI has a chance to update the feedback).
pub struct DragAndDropManager {
    /// Items that may not be dragged, e.g., because they are not movable nor copiable.
    undraggable_items: ItemCollection,

    feedback: crossbeam::atomic::AtomicCell<DragAndDropFeedback>,
}

impl DragAndDropManager {
    /// Create a [`DragAndDropManager`] by providing a list of undraggable items.
    pub fn new(undraggable_items: impl Into<ItemCollection>) -> Self {
        Self {
            undraggable_items: undraggable_items.into(),
            feedback: Default::default(),
        }
    }

    /// Set the feedback to display to the user based on drop acceptability for the UI currently
    /// hovered.
    ///
    /// By default, the feedback is unset and the pill/cursor are displayed in a "neutral" way,
    /// indicating that the current drag-and-drop payload is valid but not hovered over a related
    /// UI.
    ///
    /// If the payload type is compatible with the hovered UI element, that element should set the
    /// feedback to either [`DragAndDropFeedback::Accept`] or [`DragAndDropFeedback::Reject`], based
    /// on whether the actual payload content may meaningfully be dropped.
    ///
    /// For example, a view generally accepts a dragged entity but may occasionally reject it if
    /// it already contains it.
    pub fn set_feedback(&self, feedback: DragAndDropFeedback) {
        self.feedback.store(feedback);
    }

    /// Checks if items are draggable based on the list of undraggable items.
    pub fn are_items_draggable(&self, items: &ItemCollection) -> bool {
        self.undraggable_items
            .iter_items()
            .all(|item| !items.contains_item(item))
    }

    /// Display the currently dragged payload as a pill in the UI.
    ///
    /// This should be called once per frame.
    pub fn payload_cursor_ui(&self, ctx: &egui::Context) {
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

                let feedback = self.feedback.load();

                match feedback {
                    DragAndDropFeedback::Accept => {
                        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                        ui.set_opacity(0.8);
                    }

                    DragAndDropFeedback::Ignore => {
                        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                        ui.set_opacity(0.5);
                    }
                    DragAndDropFeedback::Reject => {
                        ctx.set_cursor_icon(egui::CursorIcon::NoDrop);
                        ui.set_opacity(0.5);
                    }
                }

                let payload_is_currently_droppable = feedback == DragAndDropFeedback::Accept;
                let response = drag_pill_frame(payload_is_currently_droppable)
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
}

fn drag_pill_frame(droppable: bool) -> egui::Frame {
    let hue = if droppable { Hue::Blue } else { Hue::Gray };

    egui::Frame {
        fill: re_ui::design_tokens().color(ColorToken::new(hue, S325)),
        stroke: egui::Stroke::new(
            1.0,
            re_ui::design_tokens().color(ColorToken::new(hue, S375)),
        ),
        rounding: 2.0.into(),
        inner_margin: egui::Margin {
            left: 6.0,
            right: 9.0,
            top: 5.0,
            bottom: 4.0,
        },
        //TODO(ab): needed to avoid the pill being cropped, not sure why?
        outer_margin: egui::Margin::same(1.0),
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
