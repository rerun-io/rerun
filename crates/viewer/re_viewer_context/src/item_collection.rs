use indexmap::IndexMap;
use itertools::Itertools as _;
use re_chunk::EntityPath;
use re_entity_db::EntityDb;
use re_log_types::StoreKind;
use re_sdk_types::external::glam;

use crate::{DataResultInteractionAddress, Item, ViewId, resolve_mono_instance_path_item};

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

/// An ordered collection of [`Item`] and optional associated context objects.
///
/// Used to store what is currently selected and/or hovered.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ItemCollection(IndexMap<Item, Option<ItemContext>>);

impl From<Item> for ItemCollection {
    #[inline]
    fn from(val: Item) -> Self {
        Self([(val, None)].into())
    }
}

impl ItemCollection {
    pub fn from_items_and_context(
        items: impl IntoIterator<Item = (Item, Option<ItemContext>)>,
    ) -> Self {
        Self(items.into_iter().collect())
    }

    /// Is this view the selected one (and no other)?
    pub fn is_view_the_only_selected(&self, needle: &ViewId) -> bool {
        let mut is_selected = false;
        for item in self.iter_items() {
            if item.view_id() == Some(*needle) {
                is_selected = true;
            } else {
                return false; // More than one view selected
            }
        }
        is_selected
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
    pub fn into_mono_instance_path_items(
        self,
        entity_db: &EntityDb,
        query: &re_chunk_store::LatestAtQuery,
    ) -> Self {
        Self(
            self.0
                .into_iter()
                .map(|(item, item_context)| {
                    (
                        resolve_mono_instance_path_item(entity_db, query, &item),
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
        if let Some(first_item) = self.first_item()
            && self
                .iter_items()
                .skip(1)
                .all(|item| std::mem::discriminant(first_item) == std::mem::discriminant(item))
        {
            return Some(first_item.kind());
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

        use re_log_channel::LogSource;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum ClipboardTextDesc {
            FilePath,
            Url,
            AppId,
            StoreId,
            EntityPath,
        }

        #[expect(clippy::match_same_arms)]
        let clipboard_texts_per_type = self
            .iter()
            .filter_map(|(item, _)| match item {
                Item::Container(_) => None,
                // TODO(gijsd): These are copyable, but we're currently unable to display a meaningful toast.
                Item::ComponentPath(_) => None,
                Item::View(_) => None,
                // TODO(lucasmerlin): Should these be copyable as URLs?
                Item::RedapServer(_) => None,
                Item::RedapEntry(_) => None,
                Item::TableId(_) => None, // TODO(grtlr): Make `TableId`s copyable too

                Item::DataSource(source) => match source {
                    LogSource::File(path) => {
                        Some((ClipboardTextDesc::FilePath, path.to_string_lossy().into()))
                    }
                    LogSource::RrdHttpStream { url, follow: _ } => {
                        Some((ClipboardTextDesc::Url, url.clone()))
                    }
                    LogSource::RrdWebEvent => None,
                    LogSource::JsChannel { .. } => None,
                    LogSource::Sdk => None,
                    LogSource::Stdin => None,
                    LogSource::RedapGrpcStream { uri, .. } => {
                        Some((ClipboardTextDesc::Url, uri.to_string()))
                    }
                    LogSource::MessageProxy(uri) => Some((ClipboardTextDesc::Url, uri.to_string())),
                },

                Item::AppId(id) => Some((ClipboardTextDesc::AppId, id.to_string())),

                // TODO(ab): it is not very meaningful to copy the `StoreId` representation, but
                // that's the best we can do for now. In the future, we should have URIs for
                // in-memory recordings, and that's what we should copy here.
                Item::StoreId(id) => Some((ClipboardTextDesc::StoreId, format!("{id:?}"))),

                Item::DataResult(DataResultInteractionAddress { instance_path, .. })
                | Item::InstancePath(instance_path) => Some((
                    ClipboardTextDesc::EntityPath,
                    instance_path.entity_path.to_string(),
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
                // Singular
                content_description.push_str(desc);
            } else {
                // Plural
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
