use re_viewer::external::re_viewer_context::SpaceViewSystemExecutionError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("edge has unknown node: {0}")]
    EdgeUnknownNode(String),

}

impl From<Error> for SpaceViewSystemExecutionError {
    fn from(val: Error) -> Self {
        // TODO(grtlr): Ensure that this is the correct error type.
        SpaceViewSystemExecutionError::DrawDataCreationError(Box::new(val))
    }
}
