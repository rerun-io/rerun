/// Different variants of errors that can happen when executing lenses.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum LensError {
    #[error("Lenses must contain at least one component")]
    MissingComponentColumns,

    #[error(transparent)]
    Transform(#[from] re_arrow_combinators::Error),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}
