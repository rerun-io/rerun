use ahash::{HashMap, HashSet};
use nohash_hasher::IntMap;
use re_data_store::{EntityPath, LogDb};
use re_log_types::{component_types::InstanceKey, EntityPathHash};

use crate::ui::{Blueprint, HistoricalSelection, SelectionHistory, SpaceView, SpaceViewId};

use super::{MultiSelection, Selection};

#[derive(Clone, Default, Debug, PartialEq)]
pub enum HoveredSpace {
    #[default]
    None,
    /// Hovering in a 2D space.
    TwoD {
        space_2d: EntityPath,
        /// Where in this 2D space (+ depth)?
        pos: glam::Vec3,
    },
    /// Hovering in a 3D space.
    ThreeD {
        /// The 3D space with the camera(s)
        space_3d: EntityPath,

        /// 2D spaces and pixel coordinates (with Z=depth)
        target_spaces: Vec<(EntityPath, Option<glam::Vec3>)>,
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

impl SelectionHighlight {
    #[inline]
    pub fn is_some(self) -> bool {
        self != SelectionHighlight::None
    }
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

impl HoverHighlight {
    #[inline]
    pub fn is_some(self) -> bool {
        self != HoverHighlight::None
    }
}

/// Combination of selection & hover highlight which can occur independently.
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct InteractionHighlight {
    pub selection: SelectionHighlight,
    pub hover: HoverHighlight,
}

impl InteractionHighlight {
    /// Any active highlight at all.
    #[inline]
    pub fn is_some(self) -> bool {
        self.selection.is_some() || self.hover.is_some()
    }

    /// Picks the stronger selection & hover highlight from two highlight descriptions.
    #[inline]
    pub fn max(&self, other: InteractionHighlight) -> Self {
        Self {
            selection: self.selection.max(other.selection),
            hover: self.hover.max(other.hover),
        }
    }
}

/// Highlights of a specific entity path in a specific space view.
///
/// Using this in bulk on many instances is faster than querying single objects.
#[derive(Default)]
pub struct SpaceViewEntityHighlight {
    overall: InteractionHighlight,
    instances: ahash::HashMap<InstanceKey, InteractionHighlight>,
}

#[derive(Copy, Clone)]
pub struct OptionalSpaceViewEntityHighlight<'a>(Option<&'a SpaceViewEntityHighlight>);

impl<'a> OptionalSpaceViewEntityHighlight<'a> {
    pub fn index_highlight(&self, instance_key: InstanceKey) -> InteractionHighlight {
        match self.0 {
            Some(entity_highlight) => entity_highlight
                .instances
                .get(&instance_key)
                .cloned()
                .unwrap_or_default()
                .max(entity_highlight.overall),
            None => InteractionHighlight::default(),
        }
    }

    pub fn any_selection_highlight(&self) -> bool {
        match self.0 {
            Some(entity_highlight) => {
                // TODO(andreas): Could easily pre-compute this!
                entity_highlight.overall.selection.is_some()
                    || entity_highlight
                        .instances
                        .values()
                        .any(|instance_highlight| instance_highlight.selection.is_some())
            }
            None => false,
        }
    }
}

/// Highlights in a specific space view.
///
/// Using this in bulk on many objects is faster than querying single objects.
#[derive(Default)]
pub struct SpaceViewHighlights {
    highlighted_entity_paths: IntMap<EntityPathHash, SpaceViewEntityHighlight>,
}

impl SpaceViewHighlights {
    pub fn entity_highlight(
        &self,
        entity_path_hash: EntityPathHash,
    ) -> OptionalSpaceViewEntityHighlight<'_> {
        OptionalSpaceViewEntityHighlight(self.highlighted_entity_paths.get(&entity_path_hash))
    }
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
    pub fn toggle_selection(&mut self, toggle_items: Vec<Selection>) {
        crate::profile_function!();

        // Make sure we preserve the order - old items kept in same order, new items added to the end.

        // All the items to toggle. If an was already selected, it will be removed from this.
        let mut toggle_items_set: HashSet<Selection> = toggle_items.iter().cloned().collect();

        let mut new_selection = self.selection.to_vec();
        new_selection.retain(|item| !toggle_items_set.remove(item));

        // Add the new items, unless they were toggling out existing items:
        new_selection.extend(
            toggle_items
                .into_iter()
                .filter(|item| toggle_items_set.contains(item)),
        );

        self.set_multi_selection(new_selection.into_iter());
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

    pub fn highlight_for_ui_element(&self, test: &Selection) -> HoverHighlight {
        let hovered = self
            .hovered_previous_frame
            .iter()
            .any(|current| match current {
                Selection::MsgId(_)
                | Selection::ComponentPath(_)
                | Selection::SpaceView(_)
                | Selection::DataBlueprintGroup(_, _) => current == test,

                Selection::InstancePath(current_space_view_id, current_instance_path) => {
                    if let Selection::InstancePath(test_space_view_id, test_instance_path) = test {
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

    pub fn highlights_for_space_view(
        &self,
        space_view_id: SpaceViewId,
        space_views: &HashMap<SpaceViewId, SpaceView>,
    ) -> SpaceViewHighlights {
        crate::profile_function!();

        let mut highlighted_entity_paths =
            IntMap::<EntityPathHash, SpaceViewEntityHighlight>::default();

        for current_selection in self.selection.iter() {
            match current_selection {
                Selection::MsgId(_) | Selection::ComponentPath(_) | Selection::SpaceView(_) => {}

                Selection::DataBlueprintGroup(group_space_view_id, group_handle) => {
                    if *group_space_view_id == space_view_id {
                        if let Some(space_view) = space_views.get(group_space_view_id) {
                            space_view.data_blueprint.visit_group_entities_recursively(
                                *group_handle,
                                &mut |entity_path: &EntityPath| {
                                    highlighted_entity_paths
                                        .entry(entity_path.hash())
                                        .or_default()
                                        .overall
                                        .selection = SelectionHighlight::SiblingSelection;
                                },
                            );
                        }
                    }
                }

                Selection::InstancePath(selected_space_view_context, selected_instance) => {
                    let highlight = if *selected_space_view_context == Some(space_view_id) {
                        SelectionHighlight::Selection
                    } else {
                        SelectionHighlight::SiblingSelection
                    };

                    let highlighted_entity = highlighted_entity_paths
                        .entry(selected_instance.entity_path.hash())
                        .or_default();

                    let highlight_target = if let Some(selected_index) =
                        selected_instance.instance_key.specific_index()
                    {
                        &mut highlighted_entity
                            .instances
                            .entry(selected_index)
                            .or_default()
                            .selection
                    } else {
                        &mut highlighted_entity.overall.selection
                    };

                    *highlight_target = (*highlight_target).max(highlight);
                }
            };
        }

        for current_hover in self.hovered_previous_frame.iter() {
            match current_hover {
                Selection::MsgId(_) | Selection::ComponentPath(_) | Selection::SpaceView(_) => {}

                Selection::DataBlueprintGroup(group_space_view_id, group_handle) => {
                    // Unlike for selected objects/data we are more picky for data blueprints with our hover highlights
                    // since they are truly local to a space view.
                    if *group_space_view_id == space_view_id {
                        if let Some(space_view) = space_views.get(group_space_view_id) {
                            space_view.data_blueprint.visit_group_entities_recursively(
                                *group_handle,
                                &mut |entity_path: &EntityPath| {
                                    highlighted_entity_paths
                                        .entry(entity_path.hash())
                                        .or_default()
                                        .overall
                                        .hover = HoverHighlight::Hovered;
                                },
                            );
                        }
                    }
                }

                Selection::InstancePath(_, selected_instance) => {
                    let highlighted_entity = highlighted_entity_paths
                        .entry(selected_instance.entity_path.hash())
                        .or_default();

                    let highlight_target = if let Some(selected_index) =
                        selected_instance.instance_key.specific_index()
                    {
                        &mut highlighted_entity
                            .instances
                            .entry(selected_index)
                            .or_default()
                            .hover
                    } else {
                        &mut highlighted_entity.overall.hover
                    };

                    *highlight_target = HoverHighlight::Hovered;
                }
            };
        }

        SpaceViewHighlights {
            highlighted_entity_paths,
        }
    }
}
