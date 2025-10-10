use re_log_types::EntryId;
use re_protos::common::v1alpha1::ext::PartitionId;

#[derive(thiserror::Error, Debug)]
#[expect(clippy::enum_variant_names)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    StoreLoadError(#[from] re_entity_db::StoreLoadError),

    #[error("Entry name '{0}' already exists")]
    DuplicateEntryNameError(String),

    #[error("Entry id '{0}' not found")]
    EntryIdNotFound(EntryId),

    #[error("Partition '{0}' not found in dataset '{1}'")]
    PartitionIdNotFound(PartitionId, EntryId),

    #[error(transparent)]
    DataFusionError(#[from] datafusion::error::DataFusionError),

    #[error("Error loading RRD: {0}")]
    RrdLoadingError(anyhow::Error),

    #[error("Failed to encode chunk key: {0}")]
    FailedToEncodeChunkKey(String),

    #[error("Failed to decode chunk key: {0}")]
    FailedToDecodeChunkKey(String),
}

impl From<Error> for tonic::Status {
    fn from(err: Error) -> Self {
        match &err {
            Error::IoError(err) => Self::internal(format!("IO error: {err:#}")),
            Error::StoreLoadError(err) => Self::internal(format!("Store load error: {err:#}")),
            Error::DuplicateEntryNameError(_) => Self::already_exists(format!("{err:#}")),

            Error::EntryIdNotFound(_) | Error::PartitionIdNotFound(_, _) => {
                Self::not_found(format!("{err:#}"))
            }

            Error::DataFusionError(err) => Self::internal(format!("DataFusion error: {err:#}")),
            Error::RrdLoadingError(err) => Self::internal(format!("{err:#}")),

            Error::FailedToDecodeChunkKey(_) => Self::invalid_argument(format!("{err:#}")),
            Error::FailedToEncodeChunkKey(_) => Self::internal(format!("{err:#}")),
        }
    }
}
