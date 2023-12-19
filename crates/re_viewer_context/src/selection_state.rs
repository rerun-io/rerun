use ahash::HashSet;
use parking_lot::Mutex;

use re_data_store::EntityPath;

use super::{Item, ItemCollection, SelectionHistory};

#[derive(Clone, Default, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SelectedSpaceContext {
    #[default]
    None,

    /// Hovering/Selecting in a 2D space.
    TwoD {
        space_2d: EntityPath,

        /// Where in this 2D space (+ depth)?
        pos: glam::Vec3,
    },

    /// Hovering/Selecting in a 3D space.
    ThreeD {
        /// The 3D space with the camera(s)
        space_3d: EntityPath,

        /// The point in 3D space that is hovered, if any.
        pos: Option<glam::Vec3>,

        /// Path of a space camera, this 3D space is viewed through.
        /// (None for a free floating Eye)
        tracked_space_camera: Option<EntityPath>,

        /// Corresponding 2D spaces and pixel coordinates (with Z=depth)
        point_in_space_cameras: Vec<(EntityPath, Option<glam::Vec3>)>,
    },
}

/// Selection highlight, sorted from weakest to strongest.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SelectionHighlight {
    /// No selection highlight at all.
    #[default]
    None,

    /// A closely related object is selected, should apply similar highlight to selection.
    /// (e.g. data in a different space view)
    SiblingSelection,

    /// Should apply selection highlight (i.e. the exact selection is highlighted).
    Selection,
}

/// Hover highlight, sorted from weakest to strongest.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum HoverHighlight {
    /// No hover highlight.
    #[default]
    None,

    /// Apply hover highlight, does *not* exclude a selection highlight.
    Hovered,
}

/// Combination of selection & hover highlight which can occur independently.
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct InteractionHighlight {
    pub selection: SelectionHighlight,
    pub hover: HoverHighlight,
}

impl InteractionHighlight {
    /// Picks the stronger selection & hover highlight from two highlight descriptions.
    #[inline]
    pub fn max(&self, other: InteractionHighlight) -> Self {
        Self {
            selection: self.selection.max(other.selection),
            hover: self.hover.max(other.hover),
        }
    }
}

/// State that makes up a selection.
#[derive(Debug, Default, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Selection {
    /// The items that were selected.
    pub items: ItemCollection,

    /// Additional spatial information about the selection.
    pub space_context: SelectedSpaceContext,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.space_context == SelectedSpaceContext::None
    }
}

impl From<ItemCollection> for Selection {
    #[inline]
    fn from(val: ItemCollection) -> Self {
        Selection {
            items: val,
            space_context: SelectedSpaceContext::None,
        }
    }
}

impl From<Item> for Selection {
    #[inline]
    fn from(val: Item) -> Self {
        ItemCollection::new(std::iter::once(val)).into()
    }
}

impl<T> From<T> for Selection
where
    T: Iterator<Item = Item>,
{
    #[inline]
    fn from(value: T) -> Self {
        ItemCollection::new(value).into()
    }
}

/// Selection and hover state.
///
/// Both hover and selection are double buffered:
/// Changes from one frame are only visible in the next frame.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ApplicationSelectionState {
    /// History of selections (what was selected previously).
    #[serde(skip)]
    pub history: Mutex<SelectionHistory>,

    /// Selection of the previous frame. Read from this.
    selection_previous_frame: Selection,

    /// Selection of the current frame. Write to this.
    #[serde(skip)]
    selection_this_frame: Mutex<Selection>,

    /// What objects are hovered? Read from this.
    #[serde(skip)]
    hovered_previous_frame: Selection,

    /// What objects are hovered? Write to this.
    #[serde(skip)]
    hovered_this_frame: Mutex<Selection>,
}

impl ApplicationSelectionState {
    /// Called at the start of each frame
    pub fn on_frame_start(&mut self, item_retain_condition: impl Fn(&Item) -> bool) {
        re_tracing::profile_function!();

        let history = self.history.get_mut();
        history.retain(&item_retain_condition);

        // Hovering needs to be refreshed every frame: If it wasn't hovered last frame, it's no longer hovered!
        self.hovered_previous_frame = std::mem::take(self.hovered_this_frame.get_mut());

        // Selection in contrast, is sticky!
        let selection_this_frame = self.selection_this_frame.get_mut();
        if selection_this_frame != &self.selection_previous_frame {
            history.update_selection(selection_this_frame);
            self.selection_previous_frame = selection_this_frame.clone();
        }
    }

    /// Selects the previous element in the history if any.
    pub fn select_previous(&self) {
        if let Some(selection) = self.history.lock().select_previous() {
            *self.selection_this_frame.lock() = selection;
        }
    }

    /// Selections the next element in the history if any.
    pub fn select_next(&self) {
        if let Some(selection) = self.history.lock().select_next() {
            *self.selection_this_frame.lock() = selection;
        }
    }

    /// Clears the current selection out.
    pub fn clear_current(&self) {
        self.set_selection(ItemCollection::default());
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Clears the selected space context if none was specified.
    pub fn set_selection(&self, items: impl Into<Selection>) {
        *self.selection_this_frame.lock() = items.into();
    }

    /// Returns the current selection.
    pub fn current(&self) -> &ItemCollection {
        &self.selection_previous_frame.items
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        &self.hovered_previous_frame.items
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered(&self, items: impl Iterator<Item = Item>) {
        self.hovered_this_frame.lock().items = ItemCollection::new(items);
    }

    /// Select passed objects unless already selected in which case they get unselected.
    ///
    /// Clears the selected space context.
    pub fn toggle_selection(&self, toggle_items: Vec<Item>) {
        re_tracing::profile_function!();

        // Make sure we preserve the order - old items kept in same order, new items added to the end.

        // All the items to toggle. If an was already selected, it will be removed from this.
        let mut toggle_items_set: HashSet<Item> = toggle_items.iter().cloned().collect();

        let mut new_selection = self.selection_previous_frame.items.to_vec();
        new_selection.retain(|item| !toggle_items_set.remove(item));

        // Add the new items, unless they were toggling out existing items:
        new_selection.extend(
            toggle_items
                .into_iter()
                .filter(|item| toggle_items_set.contains(item)),
        );

        self.set_selection(new_selection.into_iter());
    }

    pub fn selected_space_context(&self) -> &SelectedSpaceContext {
        &self.selection_previous_frame.space_context
    }

    pub fn hovered_space_context(&self) -> &SelectedSpaceContext {
        &self.hovered_previous_frame.space_context
    }

    pub fn set_hovered_space_context(&self, space: SelectedSpaceContext) {
        self.hovered_this_frame.lock().space_context = space;
    }

    pub fn set_selected_space_context(&self, space: SelectedSpaceContext) {
        self.selection_this_frame.lock().space_context = space;
    }

    pub fn highlight_for_ui_element(&self, test: &Item) -> HoverHighlight {
        let hovered = self
            .hovered_previous_frame
            .items
            .iter()
            .any(|current| match current {
                Item::ComponentPath(_)
                | Item::SpaceView(_)
                | Item::DataBlueprintGroup(_, _, _)
                | Item::Container(_) => current == test,

                Item::InstancePath(current_space_view_id, current_instance_path) => {
                    if let Item::InstancePath(test_space_view_id, test_instance_path) = test {
                        // For both space view id and instance index we want to be inclusive,
                        // but if both are set to Some, and set to different, then we count that
                        // as a miss.
                        fn either_none_or_same<T: PartialEq>(a: &Option<T>, b: &Option<T>) -> bool {
                            a.is_none() || b.is_none() || a == b
                        }

                        current_instance_path.entity_path == test_instance_path.entity_path
                            && either_none_or_same(
                                &current_instance_path.instance_key.specific_index(),
                                &test_instance_path.instance_key.specific_index(),
                            )
                            && either_none_or_same(current_space_view_id, test_space_view_id)
                    } else {
                        false
                    }
                }
            });
        if hovered {
            HoverHighlight::Hovered
        } else {
            HoverHighlight::None
        }
    }
}
