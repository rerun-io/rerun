use ahash::HashMap;
use parking_lot::Mutex;
use re_global_context::{
    CommandSender, ItemCollection, ItemContext, SystemCommand, SystemCommandSender as _,
};

use super::Item;

/// Selection highlight, sorted from weakest to strongest.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SelectionHighlight {
    /// No selection highlight at all.
    #[default]
    None,

    /// A closely related object is selected, should apply similar highlight to selection.
    /// (e.g. data in a different view)
    SiblingSelection,

    /// Should apply selection highlight (i.e. the exact selection is highlighted).
    Selection,
}

/// Hover highlight, sorted from weakest to strongest.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum HoverHighlight {
    /// No hover highlight.
    #[default]
    None,

    /// Apply hover highlight, does *not* exclude a selection highlight.
    Hovered,
}

/// Combination of selection & hover highlight which can occur independently.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct InteractionHighlight {
    pub selection: SelectionHighlight,
    pub hover: HoverHighlight,
}

impl InteractionHighlight {
    /// Picks the stronger selection & hover highlight from two highlight descriptions.
    #[inline]
    pub fn max(&self, other: Self) -> Self {
        Self {
            selection: self.selection.max(other.selection),
            hover: self.hover.max(other.hover),
        }
    }

    /// Returns true if either selection or hover is active.
    pub fn any(&self) -> bool {
        self.selection != SelectionHighlight::None || self.hover != HoverHighlight::None
    }
}

/// Selection and hover state.
///
/// Both hover and selection are double buffered:
/// Changes from one frame are only visible in the next frame.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ApplicationSelectionState {
    /// Selection of the previous frame. Read from this.
    selection_previous_frame: ItemCollection,

    /// Selection of the current frame. Write to this with [`SystemCommand::SetSelection`].
    #[serde(skip)]
    selection_this_frame: ItemCollection,

    /// What objects are hovered? Read from this.
    #[serde(skip)]
    hovered_previous_frame: ItemCollection,

    /// What objects are hovered? Write to this.
    #[serde(skip)]
    hovered_this_frame: Mutex<ItemCollection>,
}

pub enum SelectionChange<'a> {
    NoChange,
    SelectionChanged(&'a ItemCollection),
}

impl ApplicationSelectionState {
    /// Called at the start of each frame.
    pub fn on_frame_start(
        &mut self,
        item_retain_condition: impl Fn(&Item) -> bool,
        fallback_selection: Option<Item>,
    ) -> SelectionChange<'_> {
        // Use a different name so we don't get a collision in puffin.
        re_tracing::profile_scope!("SelectionState::on_frame_start");

        // Purge selection of invalid items.
        let selection_this_frame = &mut self.selection_this_frame;
        selection_this_frame.retain(|item, _| item_retain_condition(item));
        if selection_this_frame.is_empty()
            && let Some(fallback_selection) = fallback_selection
        {
            *selection_this_frame = ItemCollection::from(fallback_selection);
        }

        // Hovering needs to be refreshed every frame: If it wasn't hovered last frame, it's no longer hovered!
        self.hovered_previous_frame = std::mem::take(self.hovered_this_frame.get_mut());

        // Selection in contrast, is sticky!
        if selection_this_frame != &self.selection_previous_frame {
            self.selection_previous_frame = selection_this_frame.clone();

            SelectionChange::SelectionChanged(&*selection_this_frame)
        } else {
            SelectionChange::NoChange
        }
    }

    /// Clears the current selection out.
    pub fn clear_selection(&mut self) {
        self.set_selection(ItemCollection::default());
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Clears the selected item context if none was specified.
    pub fn set_selection(&mut self, items: impl Into<ItemCollection>) {
        self.selection_this_frame = items.into();
    }

    /// Sends a command to select the current selection + `items`.
    pub fn extend_selection(
        &self,
        items: impl Into<ItemCollection>,
        command_sender: &CommandSender,
    ) {
        let mut selections = self.selection_this_frame.clone();
        selections.extend(items.into());
        command_sender.send_system(SystemCommand::SetSelection(selections));
    }

    /// Returns the current selection.
    pub fn selected_items(&self) -> &ItemCollection {
        &self.selection_previous_frame
    }

    /// Returns the currently hovered objects.
    pub fn hovered_items(&self) -> &ItemCollection {
        &self.hovered_previous_frame
    }

    /// Set the hovered objects. Will be in [`Self::hovered_items`] on the next frame.
    pub fn set_hovered(&self, hovered: impl Into<ItemCollection>) {
        *self.hovered_this_frame.lock() = hovered.into();
    }

    /// Sends a command to select passed objects unless already selected in which case they get unselected.
    /// If however an object is already selected but now gets passed a *different* item context, it stays selected after all
    /// but with an updated context!
    pub fn toggle_selection(&self, toggle_items: ItemCollection, command_sender: &CommandSender) {
        re_tracing::profile_function!();

        let mut toggle_items_set: HashMap<Item, Option<ItemContext>> = toggle_items
            .iter()
            .map(|(item, ctx)| (item.clone(), ctx.clone()))
            .collect();

        let mut new_selection = self.selection_previous_frame.clone();

        // If an item was already selected with the exact same context remove it.
        // If an item was already selected and loses its context, remove it.
        new_selection.retain(|item, ctx| {
            if let Some(new_ctx) = toggle_items_set.get(item) {
                if new_ctx == ctx || new_ctx.is_none() {
                    toggle_items_set.remove(item);
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });

        // Update context for items that are remaining in the toggle_item_set:
        for (item, ctx) in new_selection.iter_mut() {
            if let Some(new_ctx) = toggle_items_set.get(item) {
                *ctx = new_ctx.clone();
                toggle_items_set.remove(item);
            }
        }

        // Make sure we preserve the order - old items kept in same order, new items added to the end.
        // Add the new items, unless they were toggling out existing items:
        new_selection.extend(
            toggle_items
                .into_iter()
                .filter(|(item, _)| toggle_items_set.contains_key(item)),
        );

        command_sender.send_system(SystemCommand::SetSelection(new_selection));
    }

    pub fn selection_item_contexts(&self) -> impl Iterator<Item = &ItemContext> {
        self.selection_previous_frame.iter_item_context()
    }

    pub fn hovered_item_context(&self) -> Option<&ItemContext> {
        self.hovered_previous_frame.iter_item_context().next()
    }

    pub fn highlight_for_ui_element(&self, test: &Item) -> HoverHighlight {
        let hovered = self
            .hovered_previous_frame
            .iter_items()
            .any(|current| match current {
                Item::AppId(_)
                | Item::TableId(_)
                | Item::DataSource(_)
                | Item::StoreId(_)
                | Item::View(_)
                | Item::Container(_)
                | Item::RedapEntry(_)
                | Item::RedapServer(_) => current == test,

                Item::ComponentPath(component_path) => match test {
                    Item::AppId(_)
                    | Item::TableId(_)
                    | Item::DataSource(_)
                    | Item::StoreId(_)
                    | Item::View(_)
                    | Item::Container(_)
                    | Item::RedapEntry(_)
                    | Item::RedapServer(_) => false,

                    Item::ComponentPath(test_component_path) => {
                        test_component_path == component_path
                    }

                    Item::InstancePath(test_instance_path) => {
                        !test_instance_path.instance.is_specific()
                            && test_instance_path.entity_path == component_path.entity_path
                    }
                    Item::DataResult(_, test_instance_path) => {
                        test_instance_path.entity_path == component_path.entity_path
                    }
                },

                Item::InstancePath(current_instance_path) => match test {
                    Item::AppId(_)
                    | Item::TableId(_)
                    | Item::DataSource(_)
                    | Item::StoreId(_)
                    | Item::ComponentPath(_)
                    | Item::View(_)
                    | Item::Container(_)
                    | Item::RedapEntry(_)
                    | Item::RedapServer(_) => false,

                    Item::InstancePath(test_instance_path)
                    | Item::DataResult(_, test_instance_path) => {
                        current_instance_path.entity_path == test_instance_path.entity_path
                            && either_none_or_same(
                                &current_instance_path.instance.specific_index(),
                                &test_instance_path.instance.specific_index(),
                            )
                    }
                },

                Item::DataResult(_current_view_id, current_instance_path) => match test {
                    Item::AppId(_)
                    | Item::TableId(_)
                    | Item::DataSource(_)
                    | Item::StoreId(_)
                    | Item::ComponentPath(_)
                    | Item::View(_)
                    | Item::Container(_)
                    | Item::RedapEntry(_)
                    | Item::RedapServer(_) => false,

                    Item::InstancePath(test_instance_path)
                    | Item::DataResult(_, test_instance_path) => {
                        current_instance_path.entity_path == test_instance_path.entity_path
                            && either_none_or_same(
                                &current_instance_path.instance.specific_index(),
                                &test_instance_path.instance.specific_index(),
                            )
                    }
                },
            });
        if hovered {
            HoverHighlight::Hovered
        } else {
            HoverHighlight::None
        }
    }
}

fn either_none_or_same<T: PartialEq>(a: &Option<T>, b: &Option<T>) -> bool {
    a.is_none() || b.is_none() || a == b
}
