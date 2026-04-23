use arrow::datatypes::DataType;
use re_chunk::{ComponentIdentifier, EntityPath, TimelineName};

// TODO(grtlr): Error management has grown organically and needs some cleanup.

/// Different variants of errors that can happen when executing lenses.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum LensError {
    #[error("Component '{component}' not found in chunk for entity `{entity_path}`")]
    ComponentNotFound {
        entity_path: EntityPath,
        component: ComponentIdentifier,
    },

    #[error("No output columns for input component `{input_component}`{}", target_entity.as_ref().map_or_else(String::new, |e| format!(" on target entity `{e}`")))]
    NoOutputColumns {
        input_component: ComponentIdentifier,
        target_entity: Option<EntityPath>,
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

    #[error(transparent)]
    SelectorFailed(#[from] crate::SelectorError),

    #[error("Failed to scatter existing timeline '{timeline_name}' across output rows")]
    ScatterExistingTimeFailed {
        timeline_name: TimelineName,
        #[source]
        source: arrow::error::ArrowError,
    },
}
