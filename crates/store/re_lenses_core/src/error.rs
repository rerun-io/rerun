use arrow::datatypes::DataType;
use re_chunk::{ComponentIdentifier, EntityPath, TimelineName};

/// Errors that can occur when constructing a lens via the builder API.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum LensBuilderError {
    #[error("Lens for input component `{input_component}` is missing output components")]
    MissingOutputComponent {
        input_component: ComponentIdentifier,
    },

    #[error(
        "Derive lens targets the same component as its input `{component}` on the same entity - use a mutate lens instead"
    )]
    InputEqualsOutput { component: ComponentIdentifier },

    #[error(transparent)]
    SelectorParseFailed(#[from] crate::SelectorError),
}

/// Error report from applying lenses to a chunk.
///
/// May carry a partial [`re_chunk::Chunk`] containing the columns that
/// succeeded, alongside the errors for the columns that failed.
#[derive(Debug)]
pub struct LensError {
    // Boxed to keep the `Result<Chunk, LensError>` return type small.
    inner: Box<LensErrorInner>,
}

#[derive(Debug)]
struct LensErrorInner {
    chunk: Option<re_chunk::Chunk>,
    errors: Vec<LensRuntimeError>,
}

impl LensError {
    pub(crate) fn new(chunk: Option<re_chunk::Chunk>, errors: Vec<LensRuntimeError>) -> Self {
        Self {
            inner: Box::new(LensErrorInner { chunk, errors }),
        }
    }

    /// Returns a partial chunk alongside the errors.
    pub fn with_partial_chunk(chunk: re_chunk::Chunk, errors: Vec<LensRuntimeError>) -> Self {
        Self::new(Some(chunk), errors)
    }

    /// Returns the partial chunk, if any, and consumes `self`.
    pub fn partial_chunk(self) -> Option<re_chunk::Chunk> {
        self.inner.chunk
    }

    /// Iterates over the errors that occurred during lens application.
    pub fn errors(&self) -> impl Iterator<Item = &LensRuntimeError> {
        self.inner.errors.iter()
    }
}

/// Errors that can occur when executing lenses at runtime.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum LensRuntimeError {
    #[error("Component '{component}' not found in chunk for entity `{entity_path}`")]
    ComponentNotFound {
        entity_path: EntityPath,
        component: ComponentIdentifier,
    },

    #[error("No component outputs were produced for target entity `{target_entity}`")]
    NoOutputColumnsProduced {
        input_component: ComponentIdentifier,
        target_entity: EntityPath,
    },

    #[error("Chunk validation failed: {0}")]
    ChunkValidationFailed(#[from] re_chunk::ChunkError),

    #[error(
        "Failed to apply operations to component '{component}' (entity: `{target_entity}`, input: `{input_component}`): {source}"
    )]
    ComponentOperationFailed {
        target_entity: EntityPath,
        input_component: ComponentIdentifier,
        component: ComponentIdentifier,
        #[source]
        source: Box<crate::SelectorError>,
    },

    #[error(
        "Failed to apply operations to timeline '{timeline_name}' (entity: `{target_entity}`, input: `{input_component}`): {source}"
    )]
    TimeOperationFailed {
        target_entity: EntityPath,
        input_component: ComponentIdentifier,
        timeline_name: TimelineName,
        #[source]
        source: Box<crate::SelectorError>,
    },

    #[error(
        "Invalid time column type for timeline '{timeline_name}': expected List<Int64>, got {actual_type}"
    )]
    InvalidTimeColumn {
        timeline_name: TimelineName,
        actual_type: DataType,
    },

    #[error(
        "Lens collision at entity `{entity_path}`: mutate target `{component}` already claimed by a prior lens"
    )]
    MutateCollision {
        entity_path: EntityPath,
        component: ComponentIdentifier,
    },

    #[error(
        "Lens collision at entity `{entity_path}`: derive output `{component}` already claimed by a prior lens"
    )]
    DeriveCollision {
        entity_path: EntityPath,
        component: ComponentIdentifier,
    },

    #[error("Failed to scatter existing timeline '{timeline_name}' across output rows")]
    ScatterExistingTimeFailed {
        timeline_name: TimelineName,
        #[source]
        source: arrow::error::ArrowError,
    },

    #[error(
        "Output component '{component}' for entity `{target_entity}` produced {actual} rows, \
         expected {expected} — row counts must be consistent across all output components"
    )]
    InconsistentOutputRows {
        target_entity: EntityPath,
        component: ComponentIdentifier,
        expected: usize,
        actual: usize,
    },
}
