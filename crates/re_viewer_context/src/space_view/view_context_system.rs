use ahash::HashMap;

use re_types::ComponentNameSet;

use crate::{
    NamedViewSystem, SpaceViewClassName, SpaceViewSystemExecutionError, ViewQuery, ViewSystemName,
    ViewerContext,
};

/// View context that can be used by view parts and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before view part systems.
pub trait ViewContextSystem {
    /// Returns all the component sets that the system is compatible with.
    ///
    /// If an entity path satisfies any of these sets, then the system will automatically run for
    /// that entity path.
    ///
    /// Return an empty vec to specify that the system should never run automatically for any
    /// specific entities.
    /// It may still run once per frame as part of the global context if it has been registered to
    /// do so, see [`crate::SpaceViewSystemRegistry`].
    fn compatible_component_sets(&self) -> Vec<ComponentNameSet>;

    /// Queries the data store and performs data conversions to make it ready for consumption by scene elements.
    fn execute(&mut self, ctx: &mut ViewerContext<'_>, query: &ViewQuery<'_>);

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

// TODO(jleibs): This probably needs a better name now that it includes class name
pub struct ViewContextCollection {
    pub(crate) systems: HashMap<ViewSystemName, Box<dyn ViewContextSystem>>,
    pub(crate) space_view_class_name: SpaceViewClassName,
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

    pub fn space_view_class_name(&self) -> SpaceViewClassName {
        self.space_view_class_name
    }
}
