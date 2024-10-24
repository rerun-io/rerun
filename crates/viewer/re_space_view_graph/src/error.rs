use re_log_types::EntityPath;
use re_types::datatypes;
use re_viewer_context::SpaceViewSystemExecutionError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("edge has unknown node")]
    EdgeUnknownNode,

    #[error("missing layout information for node `{1}` in entity `{0}`")]
    MissingLayoutInformation(EntityPath, datatypes::GraphNode),
}

impl From<Error> for SpaceViewSystemExecutionError {
    fn from(val: Error) -> Self {
        // TODO(grtlr): Ensure that this is the correct error type.
        Self::DrawDataCreationError(Box::new(val))
    }
}
