use re_chunk::{ComponentIdentifier, EntityPath, TimelineName};
use re_log_types::EntityPathFilter;

use crate::op::OpError;

/// Different variants of errors that can happen when executing lenses.
#[expect(missing_docs)]
#[derive(Debug, thiserror::Error)]
pub enum LensError {
    #[error(
        "Lens for input component `{input_component} with entity path filter `{input_filter:?}` is missing output components"
    )]
    MissingOutputComponent {
        input_filter: EntityPathFilter,
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

    #[error("Chunk validation failed: {0}")]
    ChunkValidationFailed(#[from] re_chunk::ChunkError),

    #[error("Failed to apply operations to component '{component}'")]
    ComponentOperationFailed {
        component: ComponentIdentifier,
        #[source]
        source: OpError,
    },

    #[error("Failed to apply operations to timeline '{timeline_name}'")]
    TimeOperationFailed {
        timeline_name: TimelineName,
        #[source]
        source: OpError,
    },

    #[error(
        "Invalid time column type for timeline '{timeline_name}': expected List<Int64>, got {actual_type}"
    )]
    InvalidTimeColumn {
        timeline_name: TimelineName,
        actual_type: arrow::datatypes::DataType,
    },

    #[error("Failed to scatter existing timeline '{timeline_name}' across output rows")]
    ScatterExistingTimeFailed {
        timeline_name: TimelineName,
        #[source]
        source: arrow::error::ArrowError,
    },
}
