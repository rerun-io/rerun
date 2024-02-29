//! Helper types for producing stable [`egui::Id`] for the purpose of handling collapsed state of
//! various UI elements.

use crate::{ContainerId, SpaceViewId};
use re_log_types::EntityPath;
use std::marker::PhantomData;

/// Defines a "scope" for the purpose of scoping stable [`egui::Id`]s.
///
/// Typically, if the same piece of data appears in two distinct UI areas, it's collapsed state
/// shouldn't be shared. The generated [`egui::Id`] should thus be scoped to the actual UI tree
/// involved. That can be done by defining a scope for each UI area, returning a unique identifier.
pub trait CollapsedIdScope {
    /// Unique identifier for the scope.
    fn identifier() -> &'static str;
}

/// A collapsed identifier.
///
/// This enum helps to generate scoped, stable [`egui::Id`]s for various pieces of data.
#[derive(Debug, Clone)]
pub enum CollapsedId<Scope: crate::collapsed_id::CollapsedIdScope> {
    Container(ContainerId),
    SpaceView(SpaceViewId),
    DataResult(SpaceViewId, EntityPath), //TODO(ab): is that sufficiently identifying?
    Entity(EntityPath),

    /// Raw [`egui::Id`].
    ///
    /// This needn't be used. Here essentially to hold the PhantomData.
    EguiId(egui::Id, PhantomData<Scope>),
}

impl<Scope: CollapsedIdScope> From<CollapsedId<Scope>> for egui::Id {
    fn from(id: CollapsedId<Scope>) -> Self {
        (&id).into()
    }
}

impl<Scope: CollapsedIdScope> From<&CollapsedId<Scope>> for egui::Id {
    fn from(id: &CollapsedId<Scope>) -> Self {
        let base_id = match id {
            CollapsedId::Container(container_id) => egui::Id::new(container_id.hash()),
            CollapsedId::SpaceView(space_view_id) => egui::Id::new(space_view_id.hash()),
            CollapsedId::DataResult(space_view_id, entity_path) => {
                egui::Id::new((space_view_id.hash(), entity_path.hash()))
            }
            CollapsedId::Entity(entity_path) => egui::Id::new(entity_path.hash64()),
            CollapsedId::EguiId(id, _) => *id,
        };

        base_id.with(Scope::identifier())
    }
}

impl<Scope: CollapsedIdScope> CollapsedId<Scope> {
    /// Convert to an [`egui::Id`].
    pub fn egui_id(&self) -> egui::Id {
        self.into()
    }

    /// Check the collapsed state for the given ID.
    pub fn is_open(&self, ctx: &egui::Context) -> Option<bool> {
        egui::collapsing_header::CollapsingState::load(ctx, self.egui_id())
            .map(|state| state.is_open())
    }

    /// Set the collapsed state for the given ID.
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

// ---
// Scopes of general interest.
//
// Note: These are defined here so to allow for easy access to the collapsed state from places
// others than the corresponding UI code (e.g. the double click "focus" behavior should be able to
// un-collapse stuff in any of these UI areas).

pub struct StreamsTreeScope;

impl CollapsedIdScope for StreamsTreeScope {
    fn identifier() -> &'static str {
        "streams_tree_ui"
    }
}

pub type StreamsCollapsedId = CollapsedId<StreamsTreeScope>;

pub struct BlueprintTreeScope;

impl CollapsedIdScope for BlueprintTreeScope {
    fn identifier() -> &'static str {
        "blueprint_tree_ui"
    }
}

pub type BlueprintCollapsedId = CollapsedId<BlueprintTreeScope>;
