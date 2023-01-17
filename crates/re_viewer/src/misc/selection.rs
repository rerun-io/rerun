use nohash_hasher::IntSet;
use re_data_store::{InstanceId, InstanceIdHash, ObjPath, ObjTypePath};
use re_log_types::{DataPath, IndexHash, MsgId, ObjPathHash};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Selection {
    None, // TODO(andreas): Once single selection is removed, None doesn't make sense anymore as it is implied by an empty MultiSelection
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
            Selection::None => write!(f, "<empty>"),
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
pub enum ObjectPathSelectionResult {
    /// No direct relation between query and what is selected.
    #[default]
    None,

    /// The entire object is in the selection.
    EntireObject,

    /// Parts of the object path are part of the selection.
    /// TODO(andreas): Optimize for the single-item case?
    Partial(IntSet<IndexHash>),
}

impl ObjectPathSelectionResult {
    pub fn is_index_selected(&self, index: IndexHash) -> bool {
        match self {
            ObjectPathSelectionResult::None => false,
            ObjectPathSelectionResult::EntireObject => true,
            ObjectPathSelectionResult::Partial(selected_indices) => {
                selected_indices.contains(&index)
            }
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
        let selection = items.collect();
        Self { selection }
    }

    /// Whether an object path is part of the selection.
    pub fn is_path_selected(&self, obj_path_hash: ObjPathHash) -> ObjectPathSelectionResult {
        let mut relevant_selected_indices = IntSet::default();

        for selection in &self.selection {
            #[allow(clippy::match_same_arms)]
            match selection {
                Selection::None => {}

                Selection::MsgId(_) => {} // TODO(andreas): Should resolve

                Selection::ObjTypePath(_) => {} // TODO(andreas): to be removed

                Selection::Instance(inst) => {
                    if inst.obj_path.hash() == obj_path_hash {
                        if let Some(index) = &inst.instance_index {
                            // TODO(andreas): Hash should be precomputed upon setting the selection.
                            relevant_selected_indices.insert(index.hash());
                        } else {
                            return ObjectPathSelectionResult::EntireObject;
                        }
                    }
                }

                Selection::DataPath(data_path) => {
                    if data_path.obj_path.hash() == obj_path_hash {
                        return ObjectPathSelectionResult::EntireObject;
                    }
                }

                Selection::Space(_) => {} // TODO(andreas): remove

                // Selecting an entire spaceview doesn't mark each object as selected.
                Selection::SpaceView(_) => {}

                Selection::SpaceViewObjPath(_, obj_path) => {
                    if obj_path.hash() == obj_path_hash {
                        return ObjectPathSelectionResult::EntireObject;
                    }
                }

                // TODO(andreas): Should resolve - "is path part of this group?"
                Selection::DataBlueprintGroup(_, _) => {}
            }
        }

        if relevant_selected_indices.is_empty() {
            ObjectPathSelectionResult::None
        } else {
            ObjectPathSelectionResult::Partial(relevant_selected_indices)
        }
    }

    /// Whether an instance is part of the selection.
    ///
    /// Should only be used if we're checking against a single instance.
    /// Avoid this when checking large arrays of instances, instead use [`Self::is_path_selected`] on the object
    /// and then [`ObjectPathSelectionQuery::is_index_in_selection`] for each index!
    pub fn is_instance_selected(&self, instance: InstanceIdHash) -> bool {
        self.is_path_selected(instance.obj_path_hash)
            .is_index_selected(instance.instance_index_hash)
    }

    pub fn is_empty(&self) -> bool {
        self.selection.is_empty()
    }

    /// The primary/first selection.
    pub fn primary(&self) -> Option<&Selection> {
        self.selection.first()
    }
}
