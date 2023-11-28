use itertools::Itertools as _;

use re_data_store::InstancePath;
use re_log_types::{ComponentPath, DataPath, EntityPath};

use crate::DataQueryId;

use super::SpaceViewId;

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
    DataBlueprintGroup(SpaceViewId, DataQueryId, EntityPath),
}

impl From<SpaceViewId> for Item {
    #[inline]
    fn from(space_view_id: SpaceViewId) -> Self {
        Self::SpaceView(space_view_id)
    }
}

impl From<ComponentPath> for Item {
    #[inline]
    fn from(component_path: ComponentPath) -> Self {
        Self::ComponentPath(component_path)
    }
}

impl From<InstancePath> for Item {
    #[inline]
    fn from(instance_path: InstancePath) -> Self {
        Self::InstancePath(None, instance_path)
    }
}

impl From<EntityPath> for Item {
    #[inline]
    fn from(entity_path: EntityPath) -> Self {
        Self::InstancePath(None, InstancePath::from(entity_path))
    }
}

impl std::str::FromStr for Item {
    type Err = re_log_types::PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let DataPath {
            entity_path,
            instance_key,
            component_name,
        } = DataPath::from_str(s)?;

        match (instance_key, component_name) {
            (Some(instance_key), Some(_component_name)) => {
                // TODO(emilk): support selecting a specific component of a specific instance.
                Err(re_log_types::PathParseError::UnexpectedInstanceKey(
                    instance_key,
                ))
            }
            (Some(instance_key), None) => Ok(Item::InstancePath(
                None,
                InstancePath::instance(entity_path, instance_key),
            )),
            (None, Some(component_name)) => Ok(Item::ComponentPath(ComponentPath {
                entity_path,
                component_name,
            })),
            (None, None) => Ok(Item::InstancePath(
                None,
                InstancePath::entity_splat(entity_path),
            )),
        }
    }
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::ComponentPath(s) => s.fmt(f),
            Item::SpaceView(s) => write!(f, "{s:?}"),
            Item::InstancePath(sid, path) => write!(f, "({sid:?}, {path})"),
            Item::DataBlueprintGroup(sid, qid, entity_path) => {
                write!(f, "({sid:?}, {qid:?}, {entity_path:?})")
            }
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
            Item::DataBlueprintGroup(_, _, _) => "Group",
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

    /// Retains elements that fulfill a certain condition.
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

/// If the given item refers to the first element of an instance with a single element, resolve to a splatted entity path.
pub fn resolve_mono_instance_path_item(
    query: &re_arrow_store::LatestAtQuery,
    store: &re_arrow_store::DataStore,
    item: &Item,
) -> Item {
    // Resolve to entity path if there's only a single instance.
    match item {
        Item::InstancePath(space_view, instance) => Item::InstancePath(
            *space_view,
            resolve_mono_instance_path(query, store, instance),
        ),
        Item::ComponentPath(_) | Item::SpaceView(_) | Item::DataBlueprintGroup(_, _, _) => {
            item.clone()
        }
    }
}

/// If the given path refers to the first element of an instance with a single element, resolve to a splatted entity path.
pub fn resolve_mono_instance_path(
    query: &re_arrow_store::LatestAtQuery,
    store: &re_arrow_store::DataStore,
    instance: &re_data_store::InstancePath,
) -> re_data_store::InstancePath {
    re_tracing::profile_function!();

    if instance.instance_key.0 == 0 {
        let Some(components) = store.all_components(&query.timeline, &instance.entity_path) else {
            // No components at all, return splatted entity.
            return re_data_store::InstancePath::entity_splat(instance.entity_path.clone());
        };
        for component in components {
            if let Some((_row_id, instances)) = re_query::get_component_with_instances(
                store,
                query,
                &instance.entity_path,
                component,
            ) {
                if instances.len() > 1 {
                    return instance.clone();
                }
            }
        }

        // All instances had only a single element or less, resolve to splatted entity.
        return re_data_store::InstancePath::entity_splat(instance.entity_path.clone());
    }

    instance.clone()
}
