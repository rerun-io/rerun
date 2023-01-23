use ahash::HashSet;
use itertools::Itertools;
use re_data_store::{LogDb, ObjPath};

use crate::ui::{Blueprint, HistoricalSelection, SelectionHistory};

use super::{MultiSelection, Selection};

#[derive(Clone, Default, Debug, PartialEq)]
pub enum HoveredSpace {
    #[default]
    None,
    /// Hovering in a 2D space.
    TwoD {
        space_2d: ObjPath,
        /// Where in this 2D space (+ depth)?
        pos: glam::Vec3,
    },
    /// Hovering in a 3D space.
    ThreeD {
        /// The 3D space with the camera(s)
        space_3d: ObjPath,

        /// 2D spaces and pixel coordinates (with Z=depth)
        target_spaces: Vec<(ObjPath, Option<glam::Vec3>)>,
    },
}

/// Selection and hover state
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct SelectionState {
    /// Currently selected things; shown in the [`crate::selection_panel::SelectionPanel`].
    ///
    /// Do not access this field directly! Use the helper methods instead, which will make sure
    /// to properly maintain the undo/redo history.
    selection: MultiSelection,

    /// History of selections (what was selected previously).
    #[serde(skip)]
    history: SelectionHistory,

    /// What objects are hovered? Read from this.
    #[serde(skip)]
    hovered_previous_frame: MultiSelection,

    /// What objects are hovered? Write to this.
    #[serde(skip)]
    hovered_this_frame: MultiSelection,

    /// What space is the pointer hovering over? Read from this.
    #[serde(skip)]
    hovered_space_previous_frame: HoveredSpace,

    /// What space is the pointer hovering over? Write to this.
    #[serde(skip)]
    hovered_space_this_frame: HoveredSpace,
}

impl SelectionState {
    /// Called at the start of each frame
    pub fn on_frame_start(&mut self, log_db: &LogDb, blueprint: &Blueprint) {
        crate::profile_function!();

        self.history.on_frame_start(log_db, blueprint);

        self.hovered_space_previous_frame =
            std::mem::replace(&mut self.hovered_space_this_frame, HoveredSpace::None);
        self.hovered_previous_frame = std::mem::take(&mut self.hovered_this_frame);
    }

    /// Selects the previous element in the history if any.
    pub fn select_previous(&mut self) -> Option<HistoricalSelection> {
        self.history.select_previous()
    }

    /// Selections the next element in the history if any.
    pub fn select_next(&mut self) -> Option<HistoricalSelection> {
        self.history.select_next()
    }

    /// Clears the current selection out.
    pub fn clear_current(&mut self) {
        self.selection = MultiSelection::default();
    }

    /// Sets a single selection, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_single_selection(&mut self, item: Selection) -> MultiSelection {
        self.set_multi_selection(std::iter::once(item))
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_multi_selection(
        &mut self,
        items: impl Iterator<Item = Selection>,
    ) -> MultiSelection {
        let new_selection = MultiSelection::new(items);
        self.history.update_selection(&new_selection);
        std::mem::replace(&mut self.selection, new_selection)
    }

    /// Returns the current selection.
    pub fn current(&self) -> &MultiSelection {
        &self.selection
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &MultiSelection {
        &self.hovered_previous_frame
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered(&mut self, items: impl Iterator<Item = Selection>) {
        self.hovered_this_frame = MultiSelection::new(items);
    }

    /// Select currently hovered objects unless already selected in which case they get unselected.
    pub fn toggle_selection(&mut self, items: impl Iterator<Item = Selection>) {
        crate::profile_function!();

        let mut selected_items = HashSet::default();
        selected_items.extend(self.selection.iter().cloned());

        // Toggling means removing if it was there and add otherwise!
        for item in items.unique() {
            if !selected_items.remove(&item) {
                selected_items.insert(item);
            }
        }

        self.set_multi_selection(selected_items.into_iter());
    }

    pub fn hovered_space(&self) -> &HoveredSpace {
        &self.hovered_space_previous_frame
    }

    pub fn set_hovered_space(&mut self, space: HoveredSpace) {
        self.hovered_space_this_frame = space;
    }

    pub fn selection_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &mut Blueprint,
    ) -> Option<MultiSelection> {
        self.history.selection_ui(ui, blueprint)
    }
}
