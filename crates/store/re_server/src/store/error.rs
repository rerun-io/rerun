use re_log_types::EntryId;

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

    #[error(transparent)]
    DataFusionError(#[from] datafusion::error::DataFusionError),

    #[error("Error loading RRD: {0}")]
    RrdLoadingError(anyhow::Error),
}

impl From<Error> for tonic::Status {
    fn from(value: Error) -> Self {
        match value {
            Error::IoError(err) => Self::internal(format!("IO error: {err:#}")),
            Error::StoreLoadError(err) => Self::internal(format!("Store load error: {err:#}")),
            Error::DuplicateEntryNameError(name) => {
                Self::already_exists(format!("Entry name already exists: {name}"))
            }
            Error::EntryIdNotFound(id) => Self::not_found(format!("Entry ID not found: {id}")),
            Error::DataFusionError(err) => Self::internal(format!("DataFusion error: {err:#}")),
            Error::RrdLoadingError(err) => Self::internal(format!("{err:#}")),
        }
    }
}
