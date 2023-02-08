use itertools::Itertools;
use re_data_store::{InstancePath, LogDb};
use re_log_types::{ComponentPath, MsgId};

use crate::ui::SpaceViewId;

/// One "thing" in the UI.
///
/// This is the granularity of what is selectable and hoverable.
///
/// A set of these is a an [`ItemCollection`].
#[derive(Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Item {
    MsgId(MsgId),
    ComponentPath(ComponentPath),
    SpaceView(SpaceViewId),
    InstancePath(Option<SpaceViewId>, InstancePath),
    DataBlueprintGroup(SpaceViewId, crate::ui::DataBlueprintGroupHandle),
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::MsgId(s) => s.fmt(f),
            Item::ComponentPath(s) => s.fmt(f),
            Item::SpaceView(s) => write!(f, "{s:?}"),
            Item::InstancePath(sid, path) => write!(f, "({sid:?}, {path})"),
            Item::DataBlueprintGroup(sid, handle) => write!(f, "({sid:?}, {handle:?})"),
        }
    }
}

impl Item {
    /// If `false`, the selection is referring to data that is no longer present.
    pub(crate) fn is_valid(&self, log_db: &LogDb, blueprint: &crate::ui::Blueprint) -> bool {
        match self {
            Item::ComponentPath(_) => true,
            Item::InstancePath(space_view_id, _) => space_view_id
                .map(|space_view_id| blueprint.viewport.space_view(&space_view_id).is_some())
                .unwrap_or(true),
            Item::MsgId(msg_id) => log_db.get_log_msg(msg_id).is_some(),
            Item::SpaceView(space_view_id) => {
                blueprint.viewport.space_view(space_view_id).is_some()
            }
            Item::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
                if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                    space_view
                        .data_blueprint
                        .group(*data_blueprint_group_handle)
                        .is_some()
                } else {
                    false
                }
            }
        }
    }

    pub fn kind(self: &Item) -> &'static str {
        match self {
            Item::MsgId(_) => "Message",
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
    selection: Vec<Item>,
}

impl ItemCollection {
    pub fn new(items: impl Iterator<Item = Item>) -> Self {
        let selection = items.unique().collect();
        Self { selection }
    }

    pub fn is_empty(&self) -> bool {
        self.selection.is_empty()
    }

    /// Number of elements in this multiselection
    pub fn len(&self) -> usize {
        self.selection.len()
    }

    /// The first selected object if any.
    pub fn first(&self) -> Option<&Item> {
        self.selection.first()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        self.selection.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = Item> {
        self.selection.into_iter()
    }

    pub fn to_vec(&self) -> Vec<Item> {
        self.selection.clone()
    }

    /// Returns true if the exact selection is part of the current selection.
    pub fn contains(&self, item: &Item) -> bool {
        self.selection.contains(item)
    }

    pub fn are_all_same_kind(&self) -> Option<&'static str> {
        if let Some(first_selection) = self.selection.first() {
            if self
                .selection
                .iter()
                .skip(1)
                .all(|item| std::mem::discriminant(first_selection) == std::mem::discriminant(item))
            {
                return Some(first_selection.kind());
            }
        }
        None
    }

    /// Remove all invalid selections.
    pub fn purge_invalid(&mut self, log_db: &LogDb, blueprint: &crate::ui::Blueprint) {
        self.selection
            .retain(|selection| selection.is_valid(log_db, blueprint));
    }
}
