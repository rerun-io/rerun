/// Different variants of errors that can happen when executing lenses.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transform(#[from] re_arrow_util::transform::Error),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error>),
}
