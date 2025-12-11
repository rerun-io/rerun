//! Utilities for determining if an entity can be added to a view.

use nohash_hasher::IntMap;
use re_entity_db::EntityTree;
use re_log_types::EntityPath;
use re_viewer_context::{DataQueryResult, ViewerContext};

use crate::ViewBlueprint;

/// Describes if an entity path can be added to a view.
#[derive(Clone, PartialEq, Eq)]
pub enum CanAddToView {
    Compatible { already_added: bool },
    No { reason: String },
}

impl Default for CanAddToView {
    fn default() -> Self {
        Self::Compatible {
            already_added: false,
        }
    }
}

impl CanAddToView {
    /// Can be generally added but view might already have this element.
    pub fn is_compatible(&self) -> bool {
        match self {
            Self::Compatible { .. } => true,
            Self::No { .. } => false,
        }
    }

    /// Can be added and view doesn't have it already.
    pub fn is_compatible_and_missing(&self) -> bool {
        self == &Self::Compatible {
            already_added: false,
        }
    }

    pub fn join(&self, other: &Self) -> Self {
        match self {
            Self::Compatible { already_added } => {
                let already_added = if let Self::Compatible {
                    already_added: already_added_other,
                } = other
                {
                    *already_added && *already_added_other
                } else {
                    *already_added
                };
                Self::Compatible { already_added }
            }
            Self::No { .. } => other.clone(),
        }
    }
}

#[derive(Clone, Default)]
pub struct EntityAddInfo {
    pub can_add: CanAddToView,
    pub can_add_self_or_descendant: CanAddToView,
}

pub fn create_entity_add_info(
    ctx: &ViewerContext<'_>,
    tree: &EntityTree,
    view: &ViewBlueprint,
    query_result: &DataQueryResult,
) -> IntMap<EntityPath, EntityAddInfo> {
    let mut meta_data: IntMap<EntityPath, EntityAddInfo> = IntMap::default();

    tree.visit_children_recursively(|entity_path| {
        let can_add: CanAddToView =
            if ctx.iter_visualizable_entities_for_view_class(view.class_identifier()).any(|(_vis, entities)| entities.contains_key(entity_path)) {
                CanAddToView::Compatible {
                    already_added: query_result.result_for_entity(entity_path).is_some(),
                }
            } else {
                // TODO(#6321): This shouldn't necessarily prevent us from adding it.
                CanAddToView::No {
                    reason: format!(
                        "Entity can't be displayed by any of the available visualizers in this class of view ({}).",
                        view.class_identifier()
                    ),
                }
            };

        if can_add.is_compatible() {
            // Mark parents aware that there is some descendant that is compatible
            let mut path = entity_path.clone();
            while let Some(parent) = path.parent() {
                let data = meta_data.entry(parent.clone()).or_default();
                data.can_add_self_or_descendant = data.can_add_self_or_descendant.join(&can_add);
                path = parent;
            }
        }

        let can_add_self_or_descendant = can_add.clone();
        meta_data.insert(
            entity_path.clone(),
            EntityAddInfo {
                can_add,
                can_add_self_or_descendant,
            },
        );
    });

    meta_data
}
