//! Helper types for producing stable [`egui::Id`] for the purpose of handling collapsed state of
//! various UI elements.

use std::hash::Hash;

use re_log_types::EntityPath;

use crate::{ContainerId, Contents, Item, ViewId};

/// The various scopes for which we want to track collapsed state.
#[derive(Debug, Clone, Copy, Hash)]
pub enum CollapseScope {
    /// Stream tree from the time panel
    StreamsTree,

    /// Stream tree from the time panel, when the filter is active
    StreamsTreeFiltered { session_id: egui::Id },

    /// The stream tree from the blueprint debug time panel
    BlueprintStreamsTree,

    /// The stream tree from the blueprint debug time panel, when the filter is active
    BlueprintStreamsTreeFiltered { session_id: egui::Id },

    /// Blueprint tree from the blueprint panel (left panel)
    BlueprintTree,

    /// Blueprint tree from the blueprint panel (left panel), when the filter is active
    BlueprintTreeFiltered { session_id: egui::Id },
}

impl CollapseScope {
    const ALL: [Self; 2] = [Self::StreamsTree, Self::BlueprintTree];

    // convenience functions

    /// Create a [`CollapsedId`] for an [`Item`] of supported kind.
    pub fn item(self, item: Item) -> Option<CollapsedId> {
        match item {
            Item::InstancePath(instance_path) => Some(self.entity(instance_path.entity_path)),
            Item::Container(container_id) => Some(self.container(container_id)),
            Item::View(view_id) => Some(self.view(view_id)),
            Item::DataResult(data_result) => {
                Some(self.data_result(data_result.view_id, data_result.instance_path.entity_path))
            }

            Item::AppId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::ComponentPath(_)
            | Item::RedapServer(_)
            | Item::RedapEntry(_)
            | Item::TableId(_) => None,
        }
    }

    /// Create a [`CollapsedId`] for a container in this scope.
    pub fn container(self, container_id: ContainerId) -> CollapsedId {
        CollapsedId {
            item: CollapseItem::Container(container_id),
            scope: self,
        }
    }

    /// Create a [`CollapsedId`] for a view in this scope.
    pub fn view(self, view_id: ViewId) -> CollapsedId {
        CollapsedId {
            item: CollapseItem::View(view_id),
            scope: self,
        }
    }

    /// Create a [`CollapsedId`] for a [`Contents`] in this scope.
    pub fn contents(self, contents: Contents) -> CollapsedId {
        match contents {
            Contents::Container(container_id) => self.container(container_id),
            Contents::View(view_id) => self.view(view_id),
        }
    }

    /// Create a [`CollapsedId`] for a data result in this scope.
    pub fn data_result(self, view_id: ViewId, entity_path: EntityPath) -> CollapsedId {
        CollapsedId {
            item: CollapseItem::DataResult(view_id, entity_path),
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
    View(ViewId),
    DataResult(ViewId, EntityPath),
    Entity(EntityPath),
}

impl CollapseItem {
    /// Set the collapsed state for the given item in every available scopes.
    pub fn set_open_all(&self, egui_ctx: &egui::Context, open: bool) {
        for scope in CollapseScope::ALL {
            let id = CollapsedId {
                item: self.clone(),
                scope,
            };
            id.set_open(egui_ctx, open);
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
        Self::new(id)
    }
}

impl CollapsedId {
    /// Convert to an [`egui::Id`].
    pub fn egui_id(&self) -> egui::Id {
        self.clone().into()
    }

    /// Check the collapsed state for the given [`CollapsedId`].
    pub fn is_open(&self, egui_ctx: &egui::Context) -> Option<bool> {
        egui::collapsing_header::CollapsingState::load(egui_ctx, self.egui_id())
            .map(|state| state.is_open())
    }

    /// Set the collapsed state for the given [`CollapsedId`].
    pub fn set_open(&self, egui_ctx: &egui::Context, open: bool) {
        let mut collapsing_state = egui::collapsing_header::CollapsingState::load_with_default_open(
            egui_ctx,
            self.egui_id(),
            false,
        );
        collapsing_state.set_open(open);
        collapsing_state.store(egui_ctx);
    }
}
