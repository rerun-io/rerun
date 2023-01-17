use re_data_store::{InstanceId, InstanceIdHash, ObjPath, ObjTypePath};
use re_log_types::{DataPath, MsgId};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Selection {
    None,
    MsgId(MsgId),
    ObjTypePath(ObjTypePath),
    Instance(InstanceId),
    DataPath(DataPath),
    Space(ObjPath),
    SpaceView(crate::ui::SpaceViewId),
    /// An object within a space-view.
    SpaceViewObjPath(crate::ui::SpaceViewId, ObjPath),
    DataBlueprintGroup(crate::ui::SpaceViewId, crate::ui::DataBlueprintGroupHandle),
}

impl std::fmt::Display for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Selection::None => write!(f, "<empty>"), // TODO(andreas): Once single selection is removed, None doesn't make sense anymore as it is implied by an empty MultiSelection
            Selection::MsgId(s) => s.fmt(f),
            Selection::ObjTypePath(s) => s.fmt(f),
            Selection::Instance(s) => s.fmt(f),
            Selection::DataPath(s) => s.fmt(f),
            Selection::Space(s) => s.fmt(f),
            Selection::SpaceView(s) => write!(f, "{s:?}"),
            Selection::SpaceViewObjPath(sid, path) => write!(f, "({sid:?}, {path})"),
            Selection::DataBlueprintGroup(sid, handle) => write!(f, "({sid:?}, {handle:?})"),
        }
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::None
    }
}

impl Selection {
    // pub fn is_none(&self) -> bool {
    //     matches!(self, Self::None)
    // }

    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_type_path(&self, needle: &ObjTypePath) -> bool {
        if let Self::ObjTypePath(hay) = self {
            hay == needle
        } else {
            false
        }
    }

    pub fn is_instance_id(&self, needle: &InstanceId) -> bool {
        if let Self::Instance(hay) = self {
            hay == needle
        } else {
            false
        }
    }

    pub fn is_obj_path(&self, needle: &ObjPath) -> bool {
        if let Self::Instance(hay) = self {
            &hay.obj_path == needle
        } else {
            false
        }
    }

    pub fn is_data_path(&self, needle: &DataPath) -> bool {
        if let Self::DataPath(hay) = self {
            hay == needle
        } else {
            false
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub enum SelectionQueryMatch {
    /// No direct relation between query and what is selected.
    #[default]
    None,

    /// Perfect match, the user selected exactly what was queried with.
    ExactObject,

    /// An instance is selected and the query object is one of them.
    InstanceInCurrentObject,

    /// A group containing this object is selected.
    BlueprintGroup,
}

impl SelectionQueryMatch {
    pub fn is_none(&self) -> bool {
        *self == SelectionQueryMatch::None
    }

    pub fn is_some(&self) -> bool {
        *self != SelectionQueryMatch::None
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct MultiSelection {
    selection: Vec<Selection>,
}

impl MultiSelection {
    pub fn is_selected(&self, instance: InstanceIdHash) -> SelectionQueryMatch {
        SelectionQueryMatch::None
    }

    pub fn set_selection(&mut self, items: impl Iterator<Item = Selection>) {
        self.selection.clear();
        self.selection.extend(items);
    }

    pub fn selection(&self) -> impl Iterator<Item = &Selection> {
        self.selection.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.selection.is_empty()
    }

    /// The primary/first selection.
    pub fn primary(&self) -> Option<&Selection> {
        self.selection.first()
    }
}
