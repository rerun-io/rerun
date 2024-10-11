use re_log_types::EntityPath;
use re_viewer::external::{re_types::datatypes, re_viewer_context::SpaceViewSystemExecutionError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("edge has unknown node: {0}")]
    EdgeUnknownNode(String),
    #[error("missing layout information for node `{1}` in entity `{0}`")]
    MissingLayoutInformation(EntityPath, datatypes::GraphNodeId),
}

impl From<Error> for SpaceViewSystemExecutionError {
    fn from(val: Error) -> Self {
        // TODO(grtlr): Ensure that this is the correct error type.
        SpaceViewSystemExecutionError::DrawDataCreationError(Box::new(val))
    }
}
