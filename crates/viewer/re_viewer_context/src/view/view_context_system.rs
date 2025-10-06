use std::any::Any;

use ahash::HashMap;

use re_types::ViewClassIdentifier;

use crate::{
    IdentifiedViewSystem, ViewContext, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    ViewerContext,
};

pub type ViewContextSystemStaticExecResult = Box<dyn Any + Send + Sync>;

/// View context that can be used by view parts and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before view part systems.
pub trait ViewContextSystem: Send + Sync {
    /// Executes once per active _type_ of `ViewContextSystem`, independent of the view's state, query, blueprint properties etc.
    ///
    /// This is run each frame once per type of view context system if the context system is used by any view.
    /// Afterwards, the such mutated context system instance gets a call to [`ViewContextSystem::execute`] for each view instance.
    ///
    /// Use this to perform any operations that are shared across all views that use this system,
    /// independent of their state, query, blueprint properties etc.
    fn execute_static(_ctx: &ViewerContext<'_>) -> ViewContextSystemStaticExecResult
    where
        Self: Sized,
    {
        Box::new(())
    }

    /// Queries the chunk store and performs data conversions to make it ready for consumption by scene elements.
    ///
    /// `self` is guaranteed to have had [`ViewContextSystem::execute_static`] invoked on it before this is called.
    /// The execution result is what is then passed on to visualizer systems.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        static_execution_result: &ViewContextSystemStaticExecResult,
    );

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
