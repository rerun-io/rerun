use itertools::Itertools;
use re_data_store::{InstanceId, LogDb};
use re_log_types::{DataPath, MsgId};

use crate::ui::SpaceViewId;

#[derive(Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Selection {
    MsgId(MsgId),
    DataPath(DataPath),
    SpaceView(SpaceViewId),
    Instance(Option<SpaceViewId>, InstanceId),
    DataBlueprintGroup(SpaceViewId, crate::ui::DataBlueprintGroupHandle),
}

impl std::fmt::Debug for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Selection::MsgId(s) => s.fmt(f),
            Selection::DataPath(s) => s.fmt(f),
            Selection::SpaceView(s) => write!(f, "{s:?}"),
            Selection::Instance(sid, path) => write!(f, "({sid:?}, {path})"),
            Selection::DataBlueprintGroup(sid, handle) => write!(f, "({sid:?}, {handle:?})"),
        }
    }
}

impl Selection {
    /// If `false`, the selection is referring to data that is no longer present.
    pub(crate) fn is_valid(&self, log_db: &LogDb, blueprint: &crate::ui::Blueprint) -> bool {
        match self {
            Selection::DataPath(_) => true,
            Selection::Instance(space_view_id, _) => space_view_id
                .map(|space_view_id| blueprint.viewport.space_view(&space_view_id).is_some())
                .unwrap_or(true),
            Selection::MsgId(msg_id) => log_db.get_log_msg(msg_id).is_some(),
            Selection::SpaceView(space_view_id) => {
                blueprint.viewport.space_view(space_view_id).is_some()
            }
            Selection::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
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

    pub fn kind(self: &Selection) -> &'static str {
        match self {
            Selection::MsgId(_) => "Message",
            Selection::Instance(space_view_id, _) => {
                if space_view_id.is_some() {
                    "Data Blueprint"
                } else {
                    "Instance"
                }
            }
            Selection::DataPath(_) => "Field",
            Selection::SpaceView(_) => "SpaceView",
            Selection::DataBlueprintGroup(_, _) => "Group",
        }
    }
}

/// Describing a single selection of things.
///
/// Immutable object, may pre-compute additional information about the selection on creation.
#[derive(Default, Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MultiSelection {
    selection: Vec<Selection>,
}

impl MultiSelection {
    pub fn new(items: impl Iterator<Item = Selection>) -> Self {
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
    pub fn first(&self) -> Option<&Selection> {
        self.selection.first()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Selection> {
        self.selection.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = Selection> {
        self.selection.into_iter()
    }

    pub fn to_vec(&self) -> Vec<Selection> {
        self.selection.clone()
    }

    /// Returns true if the exact selection is part of the current selection.
    pub fn contains(&self, selection: &Selection) -> bool {
        self.selection.contains(selection)
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
