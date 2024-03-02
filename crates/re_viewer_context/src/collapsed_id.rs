//! Helper types for producing stable [`egui::Id`] for the purpose of handling collapsed state of
//! various UI elements.

use std::hash::Hash;

use re_log_types::EntityPath;

use crate::{ContainerId, SpaceViewId};

/// The various scopes for which we want to track collapsed state.
#[derive(Debug, Clone, Copy, Hash)]
pub enum CollapseScope {
    /// Stream tree from the time panel
    StreamsTree,

    /// Blueprint tree from the blueprint panel
    BlueprintTree,
}

impl CollapseScope {
    const ALL: [CollapseScope; 2] = [CollapseScope::StreamsTree, CollapseScope::BlueprintTree];

    // convenience functions

    /// Create a [`CollapsedId`] for a container in this scope.
    pub fn container(self, container_id: ContainerId) -> CollapsedId {
        CollapsedId {
            item: CollapseItem::Container(container_id),
            scope: self,
        }
    }

    /// Create a [`CollapsedId`] for a space view in this scope.
    pub fn space_view(self, space_view_id: SpaceViewId) -> CollapsedId {
        CollapsedId {
            item: CollapseItem::SpaceView(space_view_id),
            scope: self,
        }
    }

    /// Create a [`CollapsedId`] for a data result in this scope.
    pub fn data_result(self, space_view_id: SpaceViewId, entity_path: EntityPath) -> CollapsedId {
        CollapsedId {
            item: CollapseItem::DataResult(space_view_id, entity_path),
            scope: self,
        }
    }

    /// Create a [`CollapsedId`] for an entity in this scope.
    pub fn entity(self, entity_path: EntityPath) -> CollapsedId {
        CollapsedId {
            item: CollapseItem::Entity(entity_path),
            scope: self,
        }
    }
}

/// The various kinds of items that may be represented and for which we want to track the collapsed
/// state.
#[derive(Debug, Clone, Hash)]
pub enum CollapseItem {
    Container(ContainerId),
    SpaceView(SpaceViewId),
    DataResult(SpaceViewId, EntityPath),
    Entity(EntityPath),
}

impl CollapseItem {
    /// Set the collapsed state for the given item in every available scopes.
    pub fn set_open_all(&self, ctx: &egui::Context, open: bool) {
        for scope in CollapseScope::ALL {
            let id = CollapsedId {
                item: self.clone(),
                scope,
            };
            id.set_open(ctx, open);
        }
    }
}

/// A collapsed identifier.
///
/// A `CollapsedId` resolves into a stable [`egui::Id`] for a given item and scope.
#[derive(Debug, Clone, Hash)]
pub struct CollapsedId {
    item: CollapseItem,
    scope: CollapseScope,
}

impl From<CollapsedId> for egui::Id {
    fn from(id: CollapsedId) -> Self {
        egui::Id::new(id)
    }
}

impl CollapsedId {
    /// Convert to an [`egui::Id`].
    pub fn egui_id(&self) -> egui::Id {
        self.clone().into()
    }

    /// Check the collapsed state for the given [`CollapsedId`].
    pub fn is_open(&self, ctx: &egui::Context) -> Option<bool> {
        egui::collapsing_header::CollapsingState::load(ctx, self.egui_id())
            .map(|state| state.is_open())
    }

    /// Set the collapsed state for the given [`CollapsedId`].
    pub fn set_open(&self, ctx: &egui::Context, open: bool) {
        let mut collapsing_state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ctx,
            self.egui_id(),
            false,
        );
        collapsing_state.set_open(open);
        collapsing_state.store(ctx);
    }
}
