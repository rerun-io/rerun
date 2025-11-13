use re_chunk::{ComponentIdentifier, EntityPath};

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

    // TODO(grtlr): This is very similar to the error above (just at a later stage). Can we combine those?
    //              We probably want to split builder errors from computational errors once the API stabilizes.
    #[error("No component outputs were produced for target entity `{target_entity}`")]
    NoOutputColumnsProduced {
        input_entity: EntityPath,
        input_component: ComponentIdentifier,
        target_entity: EntityPath,
    },

    #[error(transparent)]
    ChunkError(#[from] re_chunk::ChunkError),

    #[error(transparent)]
    Transform(#[from] re_arrow_combinators::Error),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}
