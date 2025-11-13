use re_chunk::ComponentIdentifier;

/// Different variants of errors that can happen when executing lenses.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum LensError {
    #[error(
        "Lens for input component `{input_component} with entity path filter `{input_filter}` is missing output components"
    )]
    MissingOutputComponent {
        input_filter: String,
        input_component: ComponentIdentifier,
    },

    #[error(transparent)]
    Transform(#[from] re_arrow_combinators::Error),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}
