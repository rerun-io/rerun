use itertools::Itertools;
use re_data_store::InstancePath;
use re_log_types::ComponentPath;

use super::{DataBlueprintGroupHandle, SpaceViewId};

/// One "thing" in the UI.
///
/// This is the granularity of what is selectable and hoverable.
///
/// A set of these is a an [`ItemCollection`].
#[derive(Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Item {
    ComponentPath(ComponentPath),
    SpaceView(SpaceViewId),
    InstancePath(Option<SpaceViewId>, InstancePath),
    DataBlueprintGroup(SpaceViewId, DataBlueprintGroupHandle),
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::ComponentPath(s) => s.fmt(f),
            Item::SpaceView(s) => write!(f, "{s:?}"),
            Item::InstancePath(sid, path) => write!(f, "({sid:?}, {path})"),
            Item::DataBlueprintGroup(sid, handle) => write!(f, "({sid:?}, {handle:?})"),
        }
    }
}

impl Item {
    pub fn kind(self: &Item) -> &'static str {
        match self {
            Item::InstancePath(space_view_id, instance_path) => {
                match (
                    instance_path.instance_key.is_specific(),
                    space_view_id.is_some(),
                ) {
                    (true, true) => "Entity Instance Blueprint",
                    (true, false) => "Entity Instance",
                    (false, true) => "Entity Blueprint",
                    (false, false) => "Entity",
                }
            }
            Item::ComponentPath(_) => "Entity Component",
            Item::SpaceView(_) => "Space View",
            Item::DataBlueprintGroup(_, _) => "Group",
        }
    }
}

/// An ordered collection of [`Item`].
///
/// Used to store what is currently selected and/or hovered.
///
/// Immutable object, may pre-compute additional information about the selection on creation.
#[derive(Default, Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ItemCollection {
    items: Vec<Item>,
}

impl ItemCollection {
    pub fn new(items: impl Iterator<Item = Item>) -> Self {
        let items = items.unique().collect();
        Self { items }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of elements in this multiselection
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// The first selected object if any.
    pub fn first(&self) -> Option<&Item> {
        self.items.first()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        self.items.iter()
    }

    pub fn to_vec(&self) -> Vec<Item> {
        self.items.clone()
    }

    /// Returns true if the exact selection is part of the current selection.
    pub fn contains(&self, item: &Item) -> bool {
        self.items.contains(item)
    }

    pub fn are_all_same_kind(&self) -> Option<&'static str> {
        if let Some(first_selection) = self.items.first() {
            if self
                .items
                .iter()
                .skip(1)
                .all(|item| std::mem::discriminant(first_selection) == std::mem::discriminant(item))
            {
                return Some(first_selection.kind());
            }
        }
        None
    }

    /// Retains elements that fulfil a certain condition.
    pub fn retain(&mut self, f: impl Fn(&Item) -> bool) {
        self.items.retain(|item| f(item));
    }
}

impl std::iter::IntoIterator for ItemCollection {
    type Item = Item;
    type IntoIter = std::vec::IntoIter<Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}
