use ahash::HashMap;

use crate::{ArchetypeDefinition, SpaceViewSystemExecutionError, ViewQuery, ViewerContext};

/// View context that can be used by view parts and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before view part systems.
pub trait ViewContextSystem {
    /// Each scene context may query several archetypes.
    ///
    /// This lists all archetypes that the context queries.
    /// A context may also query no archetypes at all and prepare caches or viewer related data instead.
    fn archetypes(&self) -> Vec<ArchetypeDefinition>;

    /// Queries the data store and performs data conversions to make it ready for consumption by scene elements.
    fn execute(&mut self, ctx: &mut ViewerContext<'_>, query: &ViewQuery<'_>);

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct ViewContextCollection {
    pub(crate) systems: HashMap<std::any::TypeId, Box<dyn ViewContextSystem>>,
}

impl ViewContextCollection {
    pub fn get<T: ViewContextSystem + Default + 'static>(
        &self,
    ) -> Result<&T, SpaceViewSystemExecutionError> {
        self.systems
            .get(&std::any::TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| {
                SpaceViewSystemExecutionError::ContextSystemNotFound(std::any::type_name::<T>())
            })
    }
}
