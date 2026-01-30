use re_mutex::Mutex;

use super::Item;
use crate::command_sender::{SelectionSource, SetSelection};
use crate::{ItemCollection, ItemContext};

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
#[derive(Default)]
pub struct ApplicationSelectionState {
    /// The selected items. Write to this with [`crate::SystemCommand::set_selection`].
    selection: ItemCollection,

    /// Has selection changed since the previous frame?
    ///
    /// Some if the selection was changed this frame.
    selection_changed: Option<SelectionSource>,

    /// What objects are hovered? Read from this.
    hovered_previous_frame: ItemCollection,

    /// What objects are hovered? Write to this.
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

        let start_len = self.selection.len();
        // Purge selection of invalid items.
        self.selection.retain(|item, _| item_retain_condition(item));

        if start_len != self.selection.len() {
            self.selection_changed = Some(SelectionSource::Other);
        }

        // Set to fallback if empty.
        if self.selection.is_empty()
            && let Some(fallback_selection) = fallback_selection
        {
            self.selection = ItemCollection::from(fallback_selection);
        }

        // Hovering needs to be refreshed every frame: If it wasn't hovered last frame, it's no longer hovered!
        self.hovered_previous_frame = std::mem::take(self.hovered_this_frame.get_mut());

        if self.selection_changed.is_some() {
            SelectionChange::SelectionChanged(&self.selection)
        } else {
            SelectionChange::NoChange
        }
    }

    pub fn on_frame_end(&mut self) {
        self.selection_changed = None;
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Clears the selected item context if none was specified.
    pub fn set_selection(&mut self, items: impl Into<SetSelection>) {
        let SetSelection { selection, source } = items.into();
        if selection != self.selection {
            self.selection_changed = Some(source);
            self.selection = selection;
        }
    }

    /// Returns the current selection.
    pub fn selected_items(&self) -> &ItemCollection {
        &self.selection
    }

    /// Returns the currently hovered objects.
    pub fn hovered_items(&self) -> &ItemCollection {
        &self.hovered_previous_frame
    }

    /// Set the hovered objects. Will be in [`Self::hovered_items`] on the next frame.
    pub fn set_hovered(&self, hovered: impl Into<ItemCollection>) {
        *self.hovered_this_frame.lock() = hovered.into();
    }

    pub fn selection_item_contexts(&self) -> impl Iterator<Item = &ItemContext> {
        self.selection.iter_item_context()
    }

    pub fn hovered_item_context(&self) -> Option<&ItemContext> {
        self.hovered_previous_frame.iter_item_context().next()
    }

    /// Returns Some if the selection changed this frame.
    pub fn selection_changed(&self) -> Option<SelectionSource> {
        self.selection_changed
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
