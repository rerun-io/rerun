use ahash::HashMap;

use crate::{ArchetypeDefinition, SpaceViewSystemExecutionError, ViewQuery, ViewerContext};

use super::view_context_system::ViewContextCollection;

/// Element of a scene derived from a single archetype query.
///
/// Is populated after scene contexts and has access to them.
pub trait ViewPartSystem {
    /// The archetype queried by this scene element.
    fn archetype(&self) -> ArchetypeDefinition;

    /// Queries the data store and performs data conversions to make it ready for display.
    ///
    /// Musn't query any data outside of the archetype.
    ///
    /// TODO(andreas): don't pass in `ViewerContext` if we want to restrict the queries here.
    /// If we want to make this restriction, then the trait-contract should be that something external
    /// to the `ViewPartSystemImpl` does the query and then passes an `ArchetypeQueryResult` into populate.
    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError>;

    /// Optionally retrieves a data store reference from the scene element.
    ///
    /// This is useful for retrieving data that is common to several parts of a [`crate::SpaceViewClass`].
    /// For example, if most scene parts produce ui elements, a concrete [`crate::SpaceViewClass`]
    /// can pick those up in its [`crate::SpaceViewClass::ui`] method by iterating over all scene parts.
    fn data(&self) -> Option<&dyn std::any::Any> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct ViewPartCollection {
    pub(crate) systems: HashMap<std::any::TypeId, Box<dyn ViewPartSystem>>,
}

impl ViewPartCollection {
    pub fn get<T: ViewPartSystem + Default + 'static>(
        &self,
    ) -> Result<&T, SpaceViewSystemExecutionError> {
        self.systems
            .get(&std::any::TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| {
                SpaceViewSystemExecutionError::PartSystemNotFound(std::any::type_name::<T>())
            })
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn ViewPartSystem> {
        self.systems.values().map(|s| s.as_ref())
    }
}
