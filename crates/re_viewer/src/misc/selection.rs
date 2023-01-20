use ahash::HashSet;
use itertools::Itertools;
use nohash_hasher::IntSet;
use re_data_store::{InstanceId, InstanceIdHash, LogDb, ObjPath};
use re_log_types::{DataPath, FieldOrComponent, IndexHash, MsgId, ObjPathHash};

use crate::ui::{DataBlueprintGroupHandle, SpaceViewId};

#[derive(Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum Selection {
    MsgId(MsgId),
    Instance(InstanceId),
    DataPath(DataPath),
    SpaceView(crate::ui::SpaceViewId),
    /// An object within a space-view.
    SpaceViewObjPath(crate::ui::SpaceViewId, ObjPath),
    DataBlueprintGroup(crate::ui::SpaceViewId, crate::ui::DataBlueprintGroupHandle),
}

impl std::fmt::Debug for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Selection::MsgId(s) => s.fmt(f),
            Selection::Instance(s) => s.fmt(f),
            Selection::DataPath(s) => s.fmt(f),
            Selection::SpaceView(s) => write!(f, "{s:?}"),
            Selection::SpaceViewObjPath(sid, path) => write!(f, "({sid:?}, {path})"),
            Selection::DataBlueprintGroup(sid, handle) => write!(f, "({sid:?}, {handle:?})"),
        }
    }
}

impl Selection {
    /// If `false`, the selection is referring to data that is no longer present.
    pub(crate) fn is_valid(&self, log_db: &LogDb, blueprint: &crate::ui::Blueprint) -> bool {
        match self {
            Selection::Instance(_) | Selection::DataPath(_) => true,
            Selection::MsgId(msg_id) => log_db.get_log_msg(msg_id).is_some(),
            Selection::SpaceView(space_view_id) | Selection::SpaceViewObjPath(space_view_id, _) => {
                blueprint.viewport.space_view(space_view_id).is_some()
            }
            Selection::DataBlueprintGroup(space_view_id, data_blueprint_group_handle) => {
                if let Some(space_view) = blueprint.viewport.space_view(space_view_id) {
                    space_view
                        .data_blueprint
                        .get_group(*data_blueprint_group_handle)
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
            Selection::Instance(_) => "Instance",
            Selection::DataPath(_) => "Field",
            Selection::SpaceView(_) => "SpaceView",
            Selection::SpaceViewObjPath(_, _) => "Data Blueprint",
            Selection::DataBlueprintGroup(_, _) => "Group",
        }
    }
}

/// Information on whether a certain object is part of a `MultiSelection`.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub enum SelectionScope<PartialInfo: std::cmp::PartialEq> {
    /// Object is not selected at all.
    #[default]
    None,

    /// Parts of the object are selected.
    ///
    /// Has information on which parts.
    Partial(PartialInfo),

    /// Indirectly selected by a parent object.
    ///
    /// We may use [`Self::None`] for a certain level of indirections.
    /// E.g. a space view selection doesn't count as indirect object path selection.
    Indirect,

    /// The exact object was explicitly selected.
    Exact,
}

impl<PartialInfo> SelectionScope<PartialInfo>
where
    PartialInfo: std::cmp::PartialEq,
{
    /// If true the exact entity was selected by the user.
    pub fn is_exact(&self) -> bool {
        self == &SelectionScope::<_>::Exact
    }

    /// True if the entity itself is included - either exactly or via parent.
    ///
    /// (i.e. partial selection of this entity isn't included)
    pub fn is_included(&self) -> bool {
        match self {
            SelectionScope::None | SelectionScope::Partial(_) => false,
            SelectionScope::Indirect | SelectionScope::Exact => true,
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct PartialObjectPathSelection {
    selected_indices: IntSet<IndexHash>,
    selected_fields_or_components: HashSet<FieldOrComponent>,
}

pub type ObjectPathSelectionScope = SelectionScope<PartialObjectPathSelection>;

impl ObjectPathSelectionScope {
    pub fn contains_index(&self, index: IndexHash) -> bool {
        match self {
            SelectionScope::None => false,
            SelectionScope::Exact | SelectionScope::Indirect => true,
            SelectionScope::Partial(PartialObjectPathSelection {
                selected_indices, ..
            }) => selected_indices.contains(&index),
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

    /// Whether an object path is part of the selection.
    pub fn check_obj_path(&self, obj_path_hash: ObjPathHash) -> ObjectPathSelectionScope {
        let mut partial = PartialObjectPathSelection::default();

        for selection in &self.selection {
            #[allow(clippy::match_same_arms)]
            match selection {
                Selection::MsgId(_) => {} // TODO(andreas): Should resolve

                Selection::Instance(inst) => {
                    if inst.obj_path.hash() == obj_path_hash {
                        if let Some(index) = &inst.instance_index {
                            // TODO(andreas): Hash should be precomputed upon setting the selection.
                            partial.selected_indices.insert(index.hash());
                        } else {
                            return SelectionScope::Exact;
                        }
                    }
                }

                Selection::DataPath(data_path) => {
                    if data_path.obj_path.hash() == obj_path_hash {
                        // TODO(andreas): Hash should be precomputed upon setting the selection - maybe just an IntSet of hashes here?
                        partial
                            .selected_fields_or_components
                            .insert(data_path.field_name);
                    }
                }

                // Selecting an entire spaceview doesn't mark each object as selected.
                Selection::SpaceView(_) => {}

                Selection::SpaceViewObjPath(_, obj_path) => {
                    if obj_path.hash() == obj_path_hash {
                        return SelectionScope::Indirect;
                    }
                }

                // TODO(andreas): Should resolve - "is path part of this group?"
                Selection::DataBlueprintGroup(_, _) => {}
            }
        }

        if partial.selected_fields_or_components.is_empty() && partial.selected_indices.is_empty() {
            SelectionScope::None
        } else {
            SelectionScope::Partial(partial)
        }
    }

    /// Whether a message id is part of the selection
    pub fn check_msg_id(&self, msg_id: MsgId) -> SelectionScope<()> {
        for selection in &self.selection {
            #[allow(clippy::match_same_arms)]
            match selection {
                Selection::MsgId(id) => {
                    if *id == msg_id {
                        return SelectionScope::Exact;
                    }
                }
                Selection::Instance(_) => {} // TODO(andreas): Check if this message logged on this instance.
                Selection::DataPath(_) => {} // TODO(andreas): Check if this message logged this data path.
                Selection::SpaceView(_) => {}
                Selection::SpaceViewObjPath(_, _) => {} // TODO(andreas): Check if this message logged on this object path.
                Selection::DataBlueprintGroup(_, _) => {} // TODO(andreas): Check if this message logged on any of the objects in this group.
            };
        }

        SelectionScope::None
    }

    /// Whether a data path is part of the selection
    pub fn check_data_path(&self, data_path: &DataPath) -> SelectionScope<()> {
        for selection in &self.selection {
            #[allow(clippy::match_same_arms)]
            match selection {
                Selection::MsgId(_) => {} // TODO(andreas): Check if this data path is on this message.
                Selection::Instance(_) => {} // TODO(andreas): Check if this instance has this data path
                Selection::DataPath(path) => {
                    if path == data_path {
                        return SelectionScope::Exact;
                    }
                }
                Selection::SpaceView(_) => {}
                Selection::SpaceViewObjPath(_, _) => {} // TODO(andreas): Check if this message logged on this object path.
                Selection::DataBlueprintGroup(_, _) => {} // TODO(andreas): Check if this message logged on any of the objects in this group.
            };
        }

        SelectionScope::None
    }

    /// Whether a space view is part of the selection
    pub fn check_space_view(&self, space_view: SpaceViewId) -> SelectionScope<()> {
        for selection in &self.selection {
            #[allow(clippy::match_same_arms)]
            match selection {
                Selection::MsgId(_) => {}
                Selection::Instance(_) => {}
                Selection::DataPath(_) => {}
                Selection::SpaceView(id) => {
                    if *id == space_view {
                        return SelectionScope::Exact;
                    }
                }
                Selection::SpaceViewObjPath(_, _) => {}
                Selection::DataBlueprintGroup(_, _) => {}
            };
        }

        SelectionScope::None
    }

    /// Whether a data blueprint group is part of the selection
    pub fn check_data_blueprint_group(
        &self,
        space_view: SpaceViewId,
        blueprint_group: DataBlueprintGroupHandle,
    ) -> SelectionScope<()> {
        for selection in &self.selection {
            #[allow(clippy::match_same_arms)]
            match selection {
                Selection::MsgId(_) => {}
                Selection::Instance(_) => {}
                Selection::DataPath(_) => {}
                Selection::SpaceView(_) => {} // Should this be `Indirect`?
                Selection::SpaceViewObjPath(_, _) => {}
                Selection::DataBlueprintGroup(sid, bid) => {
                    if *sid == space_view && *bid == blueprint_group {
                        return SelectionScope::Exact;
                    }
                }
            };
        }

        SelectionScope::None
    }

    /// Whether an instance is part of the selection.
    ///
    /// Should only be used if we're checking against a single instance.
    /// Avoid this when checking large arrays of instances, instead use [`Self::check_obj_path`] on the object
    /// and then [`SelectionScope::contains_index`] for each index!
    pub fn check_instance(&self, instance: InstanceIdHash) -> SelectionScope<()> {
        match self.check_obj_path(instance.obj_path_hash) {
            SelectionScope::None => SelectionScope::None,
            SelectionScope::Partial(partial) => {
                if partial
                    .selected_indices
                    .contains(&instance.instance_index_hash)
                {
                    SelectionScope::Exact
                } else {
                    SelectionScope::None
                }
            }
            SelectionScope::Indirect | SelectionScope::Exact => SelectionScope::Indirect,
        }
    }
}
