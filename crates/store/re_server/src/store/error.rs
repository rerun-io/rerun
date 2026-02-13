use re_log_types::EntryId;
use re_protos::common::v1alpha1::ext::SegmentId;

use crate::store::ChunkKey;

#[derive(thiserror::Error, Debug)]
#[expect(clippy::enum_variant_names)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    StoreLoadError(#[from] re_entity_db::StoreLoadError),

    #[error("Invalid entry name: {0}")]
    InvalidEntryName(String),

    #[error("Entry name '{0}' already exists")]
    DuplicateEntryNameError(String),

    #[error("Entry id '{0}' already exists")]
    DuplicateEntryIdError(EntryId),

    #[error("Entry name '{0}' not found")]
    EntryNameNotFound(String),

    #[error("Entry id '{0}' not found")]
    EntryIdNotFound(EntryId),

    #[error("Segment '{segment_id}' not found in dataset '{entry_id}'")]
    SegmentIdNotFound {
        segment_id: SegmentId,
        entry_id: EntryId,
    },

    #[error("Layer '{layer_name}' not found in segment '{segment_id}' of dataset '{entry_id}'")]
    LayerNameNotFound {
        layer_name: String,
        segment_id: SegmentId,
        entry_id: EntryId,
    },

    #[error("Layer '{0}' already exists")]
    LayerAlreadyExists(String),

    #[error("Index '{0}' not found")]
    IndexNotFound(String),

    #[error("Index '{0}' already exists")]
    IndexAlreadyExists(String),

    #[error(transparent)]
    DataFusionError(#[from] datafusion::error::DataFusionError),

    #[error(transparent)]
    ArrowError(#[from] arrow::error::ArrowError),

    #[cfg(feature = "lance")]
    #[error(transparent)]
    LanceError(#[from] lance::Error),

    #[error("Indexing error: {0}")]
    IndexingError(String),

    #[error("Error loading RRD: {0}")]
    RrdLoadingError(anyhow::Error),

    #[error("Failed to encode chunk key: {0}")]
    FailedToEncodeChunkKey(String),

    #[error("Failed to decode chunk key: {0}")]
    FailedToDecodeChunkKey(String),

    #[error("Could not find chunk: {0:#?}")]
    ChunkNotFound(ChunkKey),

    #[error("Failed to extract properties: {0:#?}")]
    FailedToExtractProperties(String),

    #[error("{0}")]
    SchemaConflict(String),

    #[error("Table storage already exists at location: {0}")]
    TableStorageAlreadyExists(String),
}

impl Error {
    #[inline]
    pub fn failed_to_extract_properties(err: impl std::error::Error) -> Self {
        Self::FailedToExtractProperties(err.to_string())
    }
}

impl From<Error> for tonic::Status {
    fn from(err: Error) -> Self {
        match &err {
            Error::IoError(err) => Self::internal(format!("IO error: {err:#}")),
            Error::StoreLoadError(err) => Self::internal(format!("Store load error: {err:#}")),

            Error::EntryIdNotFound(_)
            | Error::EntryNameNotFound(_)
            | Error::SegmentIdNotFound { .. }
            | Error::LayerNameNotFound { .. }
            | Error::IndexNotFound(_)
            | Error::ChunkNotFound(_) => Self::not_found(format!("{err:#}")),

            Error::DataFusionError(err) => Self::internal(format!("DataFusion error: {err:#}")),
            Error::ArrowError(err) => Self::internal(format!("Arrow error: {err:#}")),
            #[cfg(feature = "lance")]
            Error::LanceError(err) => Self::internal(format!("Lance error: {err:#}")),
            Error::RrdLoadingError(err) => Self::internal(format!("{err:#}")),

            Error::FailedToDecodeChunkKey(_) => Self::invalid_argument(format!("{err:#}")),
            Error::FailedToEncodeChunkKey(_) | Error::FailedToExtractProperties(_) => {
                Self::internal(format!("{err:#}"))
            }

            Error::InvalidEntryName(_) => Self::invalid_argument(format!("{err:#}")),

            Error::DuplicateEntryNameError(_)
            | Error::DuplicateEntryIdError(_)
            | Error::LayerAlreadyExists(_)
            | Error::IndexAlreadyExists(_)
            | Error::TableStorageAlreadyExists(_) => Self::already_exists(format!("{err:#}")),

            Error::IndexingError(_) => Self::internal(format!("Indexing error: {err:#}")),

            Error::SchemaConflict(_) => Self::invalid_argument(format!("{err:#}")),
        }
    }
}
