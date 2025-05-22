use ahash::HashMap;

use re_types::{ComponentDescriptorSet, ViewClassIdentifier};

use crate::{IdentifiedViewSystem, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier};

use super::view_context::ViewContext;

/// View context that can be used by view parts and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before view part systems.
pub trait ViewContextSystem: Send + Sync {
    /// Returns all the component sets that the system is compatible with.
    ///
    /// If an entity path satisfies any of these sets, then the system will automatically run for
    /// that entity path.
    ///
    /// Return an empty vec to specify that the system should never run automatically for any
    /// specific entities.
    /// It may still run once per frame as part of the global context if it has been registered to
    /// do so, see [`crate::ViewSystemRegistrator`].
    fn compatible_component_sets(&self) -> Vec<ComponentDescriptorSet>;

    /// Queries the chunk store and performs data conversions to make it ready for consumption by scene elements.
    fn execute(&mut self, ctx: &ViewContext<'_>, query: &ViewQuery<'_>);

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;
}

// TODO(jleibs): This probably needs a better name now that it includes class name
pub struct ViewContextCollection {
    pub systems: HashMap<ViewSystemIdentifier, Box<dyn ViewContextSystem>>,
    pub view_class_identifier: ViewClassIdentifier,
}

impl ViewContextCollection {
    pub fn get<T: ViewContextSystem + IdentifiedViewSystem + 'static>(
        &self,
    ) -> Result<&T, ViewSystemExecutionError> {
        self.systems
            .get(&T::identifier())
            .and_then(|s| s.as_any().downcast_ref())
            .ok_or_else(|| {
                ViewSystemExecutionError::ContextSystemNotFound(T::identifier().as_str())
            })
    }

    pub fn iter_with_identifiers(
        &self,
    ) -> impl Iterator<Item = (ViewSystemIdentifier, &dyn ViewContextSystem)> {
        self.systems.iter().map(|s| (*s.0, s.1.as_ref()))
    }

    pub fn view_class_identifier(&self) -> ViewClassIdentifier {
        self.view_class_identifier
    }
}
