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

    #[error("Duplicate output for target entity `{target_entity}`")]
    DuplicateTargetEntity { target_entity: EntityPath },

    #[error("Duplicate output for same-as-input entity")]
    DuplicateSameEntityOutput,

    #[error(transparent)]
    SelectorParseFailed(#[from] crate::SelectorError),
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

    #[error("Failed to scatter existing timeline '{timeline_name}' across output rows")]
    ScatterExistingTimeFailed {
        timeline_name: TimelineName,
        #[source]
        source: arrow::error::ArrowError,
    },
}
