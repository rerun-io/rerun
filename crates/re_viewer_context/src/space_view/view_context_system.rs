use ahash::HashMap;
use nohash_hasher::IntSet;

use re_types::ComponentName;

use crate::{
    NamedViewSystem, SpaceViewSystemExecutionError, ViewQuery, ViewSystemName, ViewerContext,
};

/// View context that can be used by view parts and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before view part systems.
pub trait ViewContextSystem {
    /// Each scene context may _require_ several different set of components in order to be
    /// instantiated.
    ///
    /// This lists all sets that the context requires.
    ///
    /// A context may also not require any components at all and merely prepare caches or viewer
    /// related data instead.
    fn all_required_components(&self) -> Vec<IntSet<ComponentName>>;

    /// Queries the data store and performs data conversions to make it ready for consumption by scene elements.
    fn execute(&mut self, ctx: &mut ViewerContext<'_>, query: &ViewQuery<'_>);

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct ViewContextCollection {
    pub(crate) systems: HashMap<ViewSystemName, Box<dyn ViewContextSystem>>,
}

impl ViewContextCollection {
    pub fn get<T: ViewContextSystem + NamedViewSystem + 'static>(
        &self,
    ) -> Result<&T, SpaceViewSystemExecutionError> {
        self.systems
            .get(&T::name())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| SpaceViewSystemExecutionError::ContextSystemNotFound(T::name().as_str()))
    }

    pub fn iter_with_names(
        &self,
    ) -> impl Iterator<Item = (ViewSystemName, &dyn ViewContextSystem)> {
        self.systems.iter().map(|s| (*s.0, s.1.as_ref()))
    }
}
