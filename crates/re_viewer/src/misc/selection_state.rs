use ahash::{HashMap, HashSet};
use egui::NumExt;
use lazy_static::lazy_static;
use nohash_hasher::IntMap;

use re_data_store::EntityPath;
use re_log_types::{component_types::InstanceKey, EntityPathHash};
use re_renderer::OutlineMaskPreference;

use crate::ui::{Blueprint, SelectionHistory, SpaceView, SpaceViewId, Viewport};

use super::{Item, ItemCollection};

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
}

#[derive(Default)]
pub struct SpaceViewOutlineMasks {
    pub overall: OutlineMaskPreference,
    pub instances: ahash::HashMap<InstanceKey, OutlineMaskPreference>,
    pub any_selection_highlight: bool,
}

lazy_static! {
    static ref SPACEVIEW_OUTLINE_MASK_NONE: SpaceViewOutlineMasks =
        SpaceViewOutlineMasks::default();
}

impl SpaceViewOutlineMasks {
    pub fn index_outline_mask(&self, instance_key: InstanceKey) -> OutlineMaskPreference {
        self.instances
            .get(&instance_key)
            .cloned()
            .unwrap_or_default()
            .with_fallback_to(self.overall)
    }
}

/// Highlights in a specific space view.
///
/// Using this in bulk on many objects is faster than querying single objects.
#[derive(Default)]
pub struct SpaceViewHighlights {
    highlighted_entity_paths: IntMap<EntityPathHash, SpaceViewEntityHighlight>,
    outlines_masks: IntMap<EntityPathHash, SpaceViewOutlineMasks>,
}

impl SpaceViewHighlights {
    pub fn entity_highlight(
        &self,
        entity_path_hash: EntityPathHash,
    ) -> OptionalSpaceViewEntityHighlight<'_> {
        OptionalSpaceViewEntityHighlight(self.highlighted_entity_paths.get(&entity_path_hash))
    }

    pub fn entity_outline_mask(&self, entity_path_hash: EntityPathHash) -> &SpaceViewOutlineMasks {
        self.outlines_masks
            .get(&entity_path_hash)
            .unwrap_or(&SPACEVIEW_OUTLINE_MASK_NONE)
    }

    pub fn any_outlines(&self) -> bool {
        !self.outlines_masks.is_empty()
    }
}

/// Selection and hover state
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SelectionState {
    /// Currently selected things; shown in the [`crate::selection_panel::SelectionPanel`].
    ///
    /// Do not access this field directly! Use the helper methods instead, which will make sure
    /// to properly maintain the undo/redo history.
    selection: ItemCollection,

    /// History of selections (what was selected previously).
    #[serde(skip)]
    history: SelectionHistory,

    /// What objects are hovered? Read from this.
    #[serde(skip)]
    hovered_previous_frame: ItemCollection,

    /// What objects are hovered? Write to this.
    #[serde(skip)]
    hovered_this_frame: ItemCollection,

    /// What space is the pointer hovering over? Read from this.
    #[serde(skip)]
    hovered_space_previous_frame: HoveredSpace,

    /// What space is the pointer hovering over? Write to this.
    #[serde(skip)]
    hovered_space_this_frame: HoveredSpace,
}

impl SelectionState {
    /// Called at the start of each frame
    pub fn on_frame_start(&mut self, blueprint: &Blueprint) {
        crate::profile_function!();

        self.history.on_frame_start(blueprint);

        self.hovered_space_previous_frame =
            std::mem::replace(&mut self.hovered_space_this_frame, HoveredSpace::None);
        self.hovered_previous_frame = std::mem::take(&mut self.hovered_this_frame);
    }

    /// Selects the previous element in the history if any.
    pub fn select_previous(&mut self) {
        if let Some(selection) = self.history.select_previous() {
            self.selection = selection;
        }
    }

    /// Selections the next element in the history if any.
    pub fn select_next(&mut self) {
        if let Some(selection) = self.history.select_next() {
            self.selection = selection;
        }
    }

    /// Clears the current selection out.
    pub fn clear_current(&mut self) {
        self.selection = ItemCollection::default();
    }

    /// Sets a single selection, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_single_selection(&mut self, item: Item) -> ItemCollection {
        self.set_multi_selection(std::iter::once(item))
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_multi_selection(&mut self, items: impl Iterator<Item = Item>) -> ItemCollection {
        let new_selection = ItemCollection::new(items);
        self.history.update_selection(&new_selection);
        std::mem::replace(&mut self.selection, new_selection)
    }

    /// Returns the current selection.
    pub fn current(&self) -> &ItemCollection {
        &self.selection
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        &self.hovered_previous_frame
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered(&mut self, items: impl Iterator<Item = Item>) {
        self.hovered_this_frame = ItemCollection::new(items);
    }

    /// Select currently hovered objects unless already selected in which case they get unselected.
    pub fn toggle_selection(&mut self, toggle_items: Vec<Item>) {
        crate::profile_function!();

        // Make sure we preserve the order - old items kept in same order, new items added to the end.

        // All the items to toggle. If an was already selected, it will be removed from this.
        let mut toggle_items_set: HashSet<Item> = toggle_items.iter().cloned().collect();

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
        re_ui: &re_ui::ReUi,
        ui: &mut egui::Ui,
        viewport: &mut Viewport,
    ) -> Option<ItemCollection> {
        self.history.selection_ui(re_ui, ui, viewport)
    }

    pub fn highlight_for_ui_element(&self, test: &Item) -> HoverHighlight {
        let hovered = self
            .hovered_previous_frame
            .iter()
            .any(|current| match current {
                Item::ComponentPath(_) | Item::SpaceView(_) | Item::DataBlueprintGroup(_, _) => {
                    current == test
                }

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

    pub fn highlights_for_space_view(
        &self,
        space_view_id: SpaceViewId,
        space_views: &HashMap<SpaceViewId, SpaceView>,
    ) -> SpaceViewHighlights {
        crate::profile_function!();

        let mut highlighted_entity_paths =
            IntMap::<EntityPathHash, SpaceViewEntityHighlight>::default();
        let mut outlines_masks = IntMap::<EntityPathHash, SpaceViewOutlineMasks>::default();

        let mut selection_mask_index: u8 = 0;
        let mut hover_mask_index: u8 = 0;
        let mut next_selection_mask = || {
            // We don't expect to overflow u8, but if we do, don't use the "background mask".
            selection_mask_index = selection_mask_index.wrapping_add(1).at_least(1);
            OutlineMaskPreference::some(0, selection_mask_index)
        };
        let mut next_hover_mask = || {
            // We don't expect to overflow u8, but if we do, don't use the "background mask".
            hover_mask_index = hover_mask_index.wrapping_add(1).at_least(1);
            OutlineMaskPreference::some(hover_mask_index, 0)
        };

        for current_selection in self.selection.iter() {
            match current_selection {
                Item::ComponentPath(_) | Item::SpaceView(_) => {}

                Item::DataBlueprintGroup(group_space_view_id, group_handle) => {
                    if *group_space_view_id == space_view_id {
                        if let Some(space_view) = space_views.get(group_space_view_id) {
                            // Everything in the same group should receive the same selection outline.
                            // (Due to the way outline masks work in re_renderer, we can't leave the hover channel empty)
                            let selection_mask = next_selection_mask();

                            space_view.data_blueprint.visit_group_entities_recursively(
                                *group_handle,
                                &mut |entity_path: &EntityPath| {
                                    highlighted_entity_paths
                                        .entry(entity_path.hash())
                                        .or_default()
                                        .overall
                                        .selection = SelectionHighlight::SiblingSelection;
                                    let outline_mask_ids =
                                        outlines_masks.entry(entity_path.hash()).or_default();
                                    outline_mask_ids.overall =
                                        selection_mask.with_fallback_to(outline_mask_ids.overall);
                                    outline_mask_ids.any_selection_highlight = true;
                                },
                            );
                        }
                    }
                }

                Item::InstancePath(selected_space_view_context, selected_instance) => {
                    {
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
                    {
                        let outline_mask_ids = outlines_masks
                            .entry(selected_instance.entity_path.hash())
                            .or_default();
                        outline_mask_ids.any_selection_highlight = true;
                        let outline_mask_target = if let Some(selected_index) =
                            selected_instance.instance_key.specific_index()
                        {
                            outline_mask_ids
                                .instances
                                .entry(selected_index)
                                .or_default()
                        } else {
                            &mut outline_mask_ids.overall
                        };
                        *outline_mask_target =
                            next_selection_mask().with_fallback_to(*outline_mask_target);
                    }
                }
            };
        }

        for current_hover in self.hovered_previous_frame.iter() {
            match current_hover {
                Item::ComponentPath(_) | Item::SpaceView(_) => {}

                Item::DataBlueprintGroup(group_space_view_id, group_handle) => {
                    // Unlike for selected objects/data we are more picky for data blueprints with our hover highlights
                    // since they are truly local to a space view.
                    if *group_space_view_id == space_view_id {
                        if let Some(space_view) = space_views.get(group_space_view_id) {
                            // Everything in the same group should receive the same selection outline.
                            let hover_mask = next_hover_mask();

                            space_view.data_blueprint.visit_group_entities_recursively(
                                *group_handle,
                                &mut |entity_path: &EntityPath| {
                                    highlighted_entity_paths
                                        .entry(entity_path.hash())
                                        .or_default()
                                        .overall
                                        .hover = HoverHighlight::Hovered;
                                    let mask =
                                        outlines_masks.entry(entity_path.hash()).or_default();
                                    mask.overall = hover_mask.with_fallback_to(mask.overall);
                                },
                            );
                        }
                    }
                }

                Item::InstancePath(_, selected_instance) => {
                    {
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
                    {
                        let outlined_entity = outlines_masks
                            .entry(selected_instance.entity_path.hash())
                            .or_default();
                        let outline_mask_target = if let Some(selected_index) =
                            selected_instance.instance_key.specific_index()
                        {
                            outlined_entity.instances.entry(selected_index).or_default()
                        } else {
                            &mut outlined_entity.overall
                        };
                        *outline_mask_target =
                            next_hover_mask().with_fallback_to(*outline_mask_target);
                    }
                }
            };
        }

        SpaceViewHighlights {
            highlighted_entity_paths,
            outlines_masks,
        }
    }
}
