use std::any::Any;

use ahash::HashMap;
use re_chunk_store::MissingChunkReporter;
use re_sdk_types::ViewClassIdentifier;

use crate::{
    IdentifiedViewSystem, ViewContext, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
    ViewerContext, VisualizerExecutionOutput,
};

pub type ViewContextSystemOncePerFrameResult = Box<dyn Any + Send + Sync>;

/// View context that can be used by view parts and ui methods to retrieve information about the scene as a whole.
///
/// Is always populated before view part systems.
pub trait ViewContextSystem: Send + Sync + Any {
    /// Executes once per active _type_ of [`ViewContextSystem`], independent of the view's state, query, blueprint properties etc.
    ///
    /// This is run each frame once per type of view context system if the context system is used by any view.
    /// The returned [`ViewContextSystemOncePerFrameResult`] is then passed to [`ViewContextSystem::execute`] for each view instance.
    ///
    /// Use this to perform any operations that are shared across all views that use this system,
    /// independent of their state, query, blueprint properties etc.
    fn execute_once_per_frame(_ctx: &ViewerContext<'_>) -> ViewContextSystemOncePerFrameResult
    where
        Self: Sized,
    {
        Box::new(())
    }

    /// Queries the chunk store and performs data conversions to make it ready for consumption by scene elements.
    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        missing_chunk_reporter: &MissingChunkReporter,
        query: &ViewQuery<'_>,
        one_per_frame_execution_result: &ViewContextSystemOncePerFrameResult,
    );
}

/// State stored per [`ViewContextSystem`] as a result of the last
/// call to [`ViewContextSystem::execute`].
#[derive(Default, Debug)]
pub struct ViewSystemState {
    pub any_missing_chunks: bool,
}

// TODO(jleibs): This probably needs a better name now that it includes class name
pub struct ViewContextCollection {
    pub systems: HashMap<ViewSystemIdentifier, (Box<dyn ViewContextSystem>, ViewSystemState)>,
    pub view_class_identifier: ViewClassIdentifier,
}

impl ViewContextCollection {
    /// The `output` is only there so we can report if there are any missing chunks
    pub fn get<T: ViewContextSystem + IdentifiedViewSystem + 'static>(
        &self,
        output: &VisualizerExecutionOutput,
    ) -> Result<&T, ViewSystemExecutionError> {
        self.get_and_report_missing(output.missing_chunk_reporter())
    }

    pub fn get_and_report_missing<T: ViewContextSystem + IdentifiedViewSystem + 'static>(
        &self,
        missing_chunk_reporter: &MissingChunkReporter,
    ) -> Result<&T, ViewSystemExecutionError> {
        self.systems
            .get(&T::identifier())
            .and_then(|(system, state)| {
                if state.any_missing_chunks {
                    missing_chunk_reporter.report_missing_chunk();
                }
                (system.as_ref() as &dyn Any).downcast_ref()
            })
            .ok_or_else(|| {
                ViewSystemExecutionError::ContextSystemNotFound(T::identifier().as_str())
            })
    }

    pub fn iter_with_identifiers(
        &self,
    ) -> impl Iterator<Item = (ViewSystemIdentifier, &dyn ViewContextSystem)> {
        self.systems
            .iter()
            .map(|(id, (system, _state))| (*id, system.as_ref()))
    }

    pub fn view_class_identifier(&self) -> ViewClassIdentifier {
        self.view_class_identifier
    }
}
