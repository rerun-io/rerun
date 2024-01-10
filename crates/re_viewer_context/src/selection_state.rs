use ahash::HashMap;
use parking_lot::Mutex;

use re_entity_db::EntityPath;

use crate::{item::resolve_mono_instance_path_item, ViewerContext};

use super::{Item, SelectionHistory};

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SelectedSpaceContext {
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

/// An ordered collection of [`Item`] and optional associated selected space context objects.
///
/// Used to store what is currently selected and/or hovered.
#[derive(Debug, Default, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Selection(pub Vec<(Item, Option<SelectedSpaceContext>)>);

impl From<Item> for Selection {
    #[inline]
    fn from(val: Item) -> Self {
        Selection(vec![(val, None)])
    }
}

impl<T> From<T> for Selection
where
    T: Iterator<Item = Item>,
{
    #[inline]
    fn from(value: T) -> Self {
        Selection(value.map(|item| (item, None)).collect())
    }
}

impl std::ops::Deref for Selection {
    type Target = Vec<(Item, Option<SelectedSpaceContext>)>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Selection {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Selection {
    /// For each item in this selection, if it refers to the first element of an instance with a single element, resolve it to a splatted entity path.
    pub fn resolve_mono_instance_path_items(&mut self, ctx: &ViewerContext<'_>) {
        for (item, _) in self.iter_mut() {
            *item =
                resolve_mono_instance_path_item(&ctx.current_query(), ctx.entity_db.store(), item);
        }
    }

    /// The first selected object if any.
    pub fn first_item(&self) -> Option<&Item> {
        self.0.first().map(|(item, _)| item)
    }

    pub fn iter_items(&self) -> impl Iterator<Item = &Item> {
        self.0.iter().map(|(item, _)| item)
    }

    pub fn iter_space_context(&self) -> impl Iterator<Item = &SelectedSpaceContext> {
        self.0
            .iter()
            .filter_map(|(_, space_context)| space_context.as_ref())
    }

    /// Returns true if the exact selection is part of the current selection.
    pub fn contains_item(&self, needle: &Item) -> bool {
        self.0.iter().any(|(item, _)| item == needle)
    }

    pub fn are_all_items_same_kind(&self) -> Option<&'static str> {
        if let Some(first_item) = self.first_item() {
            if self
                .iter_items()
                .skip(1)
                .all(|item| std::mem::discriminant(first_item) == std::mem::discriminant(item))
            {
                return Some(first_item.kind());
            }
        }
        None
    }

    /// Retains elements that fulfill a certain condition.
    pub fn retain(&mut self, f: impl Fn(&Item) -> bool) {
        self.0.retain(|(item, _)| f(item));
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
        // Use a different name so we don't get a collision in puffin.
        re_tracing::profile_scope!("SelectionState::on_frame_start");

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
        self.set_selection(Selection::default());
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Clears the selected space context if none was specified.
    pub fn set_selection(&self, items: impl Into<Selection>) {
        *self.selection_this_frame.lock() = items.into();
    }

    /// Returns the current selection.
    pub fn current(&self) -> &Selection {
        &self.selection_previous_frame
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &Selection {
        &self.hovered_previous_frame
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered(&self, hovered: impl Into<Selection>) {
        *self.hovered_this_frame.lock() = hovered.into();
    }

    /// Select passed objects unless already selected in which case they get unselected.
    /// If however an object is already selected but now gets passed a *different* selected space context, it stays selected after all
    /// but with an updated selected space context!
    pub fn toggle_selection(&self, toggle_items: Selection) {
        re_tracing::profile_function!();

        let mut toggle_items_set: HashMap<Item, Option<SelectedSpaceContext>> =
            toggle_items.iter().cloned().collect();

        let mut new_selection = self.selection_previous_frame.clone();

        // If an item was already selected with the exact same context remove it.
        // If an item was already selected and loses its context, remove it.
        new_selection.0.retain(|(item, ctx)| {
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
                .0
                .into_iter()
                .filter(|(item, _)| toggle_items_set.contains_key(item)),
        );

        *self.selection_this_frame.lock() = new_selection;
    }

    pub fn selected_space_context(&self) -> impl Iterator<Item = &SelectedSpaceContext> {
        self.selection_previous_frame.iter_space_context()
    }

    pub fn hovered_space_context(&self) -> Option<&SelectedSpaceContext> {
        self.hovered_previous_frame.iter_space_context().next()
    }

    pub fn highlight_for_ui_element(&self, test: &Item) -> HoverHighlight {
        let hovered = self
            .hovered_previous_frame
            .iter_items()
            .any(|current| match current {
                Item::StoreId(_)
                | Item::ComponentPath(_)
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
