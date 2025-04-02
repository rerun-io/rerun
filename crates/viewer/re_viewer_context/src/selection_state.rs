use ahash::HashMap;
use indexmap::IndexMap;
use itertools::Itertools as _;
use parking_lot::Mutex;

use crate::{global_context::resolve_mono_instance_path_item, ViewerContext};
use re_entity_db::EntityPath;
use re_log_types::StoreKind;

use super::Item;

/// Context information that a view might attach to an item from [`ItemCollection`] and useful
/// for how a selection might be displayed and interacted with.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum ItemContext {
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

        /// Path to an entity that is currently tracked by the eye-camera.
        /// (None for a free floating Eye)
        tracked_entity: Option<EntityPath>,

        /// Corresponding 2D spaces and pixel coordinates (with Z=depth)
        point_in_space_cameras: Vec<(EntityPath, Option<glam::Vec3>)>,
    },

    /// Hovering/selecting in one of the streams trees.
    StreamsTree {
        /// Which store does this streams tree correspond to?
        store_kind: StoreKind,

        /// The current entity filter session id, if any.
        filter_session_id: Option<egui::Id>,
    },

    /// Hovering/selecting in the blueprint tree.
    BlueprintTree {
        /// The current entity filter session id, if any.
        filter_session_id: Option<egui::Id>,
    },
}

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

/// An ordered collection of [`Item`] and optional associated context objects.
///
/// Used to store what is currently selected and/or hovered.
#[derive(Debug, Default, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ItemCollection(IndexMap<Item, Option<ItemContext>>);

impl From<Item> for ItemCollection {
    #[inline]
    fn from(val: Item) -> Self {
        Self([(val, None)].into())
    }
}

impl<T> From<T> for ItemCollection
where
    T: Iterator<Item = (Item, Option<ItemContext>)>,
{
    #[inline]
    fn from(value: T) -> Self {
        Self(value.collect())
    }
}

impl IntoIterator for ItemCollection {
    type Item = (Item, Option<ItemContext>);
    type IntoIter = indexmap::map::IntoIter<Item, Option<ItemContext>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl ItemCollection {
    /// For each item in this selection, if it refers to the first element of an instance with a
    /// single element, resolve it to a unindexed entity path.
    pub fn into_mono_instance_path_items(self, ctx: &ViewerContext<'_>) -> Self {
        Self(
            self.0
                .into_iter()
                .map(|(item, item_context)| {
                    (
                        resolve_mono_instance_path_item(
                            ctx.recording(),
                            &ctx.current_query(),
                            &item,
                        ),
                        item_context,
                    )
                })
                .collect(),
        )
    }

    /// The first selected object if any.
    pub fn first_item(&self) -> Option<&Item> {
        self.0.keys().next()
    }

    /// Check if the selection contains a single item and returns it if so.
    pub fn single_item(&self) -> Option<&Item> {
        if self.len() == 1 {
            self.first_item()
        } else {
            None
        }
    }

    pub fn iter_items(&self) -> impl Iterator<Item = &Item> {
        self.0.keys()
    }

    pub fn iter_item_context(&self) -> impl Iterator<Item = &ItemContext> {
        self.0
            .iter()
            .filter_map(|(_, item_context)| item_context.as_ref())
    }

    pub fn context_for_item(&self, item: &Item) -> Option<&ItemContext> {
        self.0.get(item).and_then(Option::as_ref)
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
    pub fn retain(&mut self, f: impl FnMut(&Item, &mut Option<ItemContext>) -> bool) {
        self.0.retain(f);
    }

    /// Returns the number of items in the selection.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the selection is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over the items and their selected context.
    pub fn iter(&self) -> impl Iterator<Item = (&Item, &Option<ItemContext>)> {
        self.0.iter()
    }

    /// Returns a mutable iterator over the items and their selected context.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Item, &mut Option<ItemContext>)> {
        self.0.iter_mut()
    }

    /// Extend the selection with more items.
    pub fn extend(&mut self, other: impl IntoIterator<Item = (Item, Option<ItemContext>)>) {
        self.0.extend(other);
    }

    /// Tries to copy a description of the selection to the clipboard.
    ///
    /// Only certain elements are copyable right now.
    pub fn copy_to_clipboard(&self, egui_ctx: &egui::Context) {
        if self.is_empty() {
            return;
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum ClipboardTextDesc {
            FilePath,
            Url,
            AppId,
            StoreId,
            EntityPath,
        }

        #[allow(clippy::match_same_arms)]
        let clipboard_texts_per_type = self
            .iter()
            .filter_map(|(item, _)| match item {
                Item::Container(_) => None,
                Item::View(_) => None,
                Item::TableId(_) => None, // TODO(grtlr): Make `TableId`s copyable too

                Item::DataSource(source) => match source {
                    re_smart_channel::SmartChannelSource::File(path) => {
                        Some((ClipboardTextDesc::FilePath, path.to_string_lossy().into()))
                    }
                    re_smart_channel::SmartChannelSource::RrdHttpStream { url, follow: _ } => {
                        Some((ClipboardTextDesc::Url, url.clone()))
                    }
                    re_smart_channel::SmartChannelSource::RrdWebEventListener => None,
                    re_smart_channel::SmartChannelSource::JsChannel { .. } => None,
                    re_smart_channel::SmartChannelSource::Sdk => None,
                    re_smart_channel::SmartChannelSource::Stdin => None,
                    re_smart_channel::SmartChannelSource::RedapGrpcStream(endpoint) => {
                        Some((ClipboardTextDesc::Url, endpoint.to_string()))
                    }
                    re_smart_channel::SmartChannelSource::MessageProxy { url } => {
                        Some((ClipboardTextDesc::Url, url.clone()))
                    }
                },

                Item::AppId(id) => Some((ClipboardTextDesc::AppId, id.to_string())),
                Item::StoreId(id) => Some((ClipboardTextDesc::StoreId, id.to_string())),

                Item::DataResult(_, instance_path) | Item::InstancePath(instance_path) => Some((
                    ClipboardTextDesc::EntityPath,
                    instance_path.entity_path.to_string(),
                )),
                Item::ComponentPath(component_path) => Some((
                    ClipboardTextDesc::EntityPath,
                    component_path.entity_path.to_string(),
                )),
            })
            .chunk_by(|(desc, _)| *desc);

        let mut clipboard_text = String::new();
        let mut content_description = String::new();

        for (desc, entries) in &clipboard_texts_per_type {
            let entries = entries.map(|(_, text)| text).collect_vec();

            let desc = match desc {
                ClipboardTextDesc::FilePath => "file path",
                ClipboardTextDesc::Url => "URL",
                ClipboardTextDesc::AppId => "app id",
                ClipboardTextDesc::StoreId => "store id",
                ClipboardTextDesc::EntityPath => "entity path",
            };
            if !content_description.is_empty() {
                content_description.push_str(", ");
            }
            if entries.len() == 1 {
                content_description.push_str(desc);
            } else {
                content_description.push_str(&format!("{desc}s"));
            }

            let texts = entries.into_iter().join("\n");
            if !clipboard_text.is_empty() {
                clipboard_text.push('\n');
            }
            clipboard_text.push_str(&texts);
        }

        if !clipboard_text.is_empty() {
            re_log::info!(
                "Copied {content_description} to clipboard:\n{}",
                &clipboard_text
            );
            egui_ctx.copy_text(clipboard_text);
        }
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

    /// Selection of the current frame. Write to this.
    #[serde(skip)]
    selection_this_frame: Mutex<ItemCollection>,

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
        let selection_this_frame = self.selection_this_frame.get_mut();
        selection_this_frame.retain(|item, _| item_retain_condition(item));
        if selection_this_frame.is_empty() {
            if let Some(fallback_selection) = fallback_selection {
                *selection_this_frame = ItemCollection::from(fallback_selection);
            }
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
    pub fn clear_selection(&self) {
        self.set_selection(ItemCollection::default());
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Clears the selected item context if none was specified.
    pub fn set_selection(&self, items: impl Into<ItemCollection>) {
        *self.selection_this_frame.lock() = items.into();
    }

    /// Extend the selection with the provided items.
    pub fn extend_selection(&self, items: impl Into<ItemCollection>) {
        self.selection_this_frame.lock().extend(items.into());
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

    /// Select passed objects unless already selected in which case they get unselected.
    /// If however an object is already selected but now gets passed a *different* item context, it stays selected after all
    /// but with an updated context!
    pub fn toggle_selection(&self, toggle_items: ItemCollection) {
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
                .0
                .into_iter()
                .filter(|(item, _)| toggle_items_set.contains_key(item)),
        );

        *self.selection_this_frame.lock() = new_selection;
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
                | Item::Container(_) => current == test,

                Item::ComponentPath(component_path) => match test {
                    Item::AppId(_)
                    | Item::TableId(_)
                    | Item::DataSource(_)
                    | Item::StoreId(_)
                    | Item::View(_)
                    | Item::Container(_) => false,

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
                    | Item::Container(_) => false,

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
                    | Item::Container(_) => false,

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
