//! Provide query-centric access to the [`re_data_store`].

// TODO: do bring back the table previews

// TODO: we need e2e examples that showcase (use 3d point cloud for both):
// - latest_at: query + clamped_zip
// - range: query + range_zip + clamped_zip

mod clamped_zip;
mod latest_at;
mod promise;
mod range;
mod range_zip;
mod util;

pub use self::clamped_zip::{clamped_zip_1x1, clamped_zip_1x2};
pub use self::latest_at::{latest_at, LatestAtComponentResults, LatestAtResults};
pub use self::promise::{Promise, PromiseResolver, PromiseResult};
pub use self::range::{range, RangeComponentResults, RangeResults};
pub use self::range_zip::{range_zip_1x1, range_zip_1x2};
pub use self::util::{
    query_with_history, ExtraQueryHistory, VisibleHistory, VisibleHistoryBoundary,
};

// ---

#[derive(Debug, Clone, Copy)]
pub struct ComponentNotFoundError(pub re_types_core::ComponentName);

impl std::fmt::Display for ComponentNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Could not find component: {}", self.0))
    }
}

impl std::error::Error for ComponentNotFoundError {}

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Tried to access a column that doesn't exist")]
    BadAccess,

    #[error("Could not find primary component: {0}")]
    PrimaryNotFound(re_types_core::ComponentName),

    #[error(transparent)]
    ComponentNotFound(#[from] ComponentNotFoundError),

    #[error("Tried to access component of type '{actual:?}' using component '{requested:?}'")]
    TypeMismatch {
        actual: re_types_core::ComponentName,
        requested: re_types_core::ComponentName,
    },

    #[error("Error with one or more the underlying data cells: {0}")]
    DataCell(#[from] re_log_types::DataCellError),

    #[error("Error deserializing: {0}")]
    DeserializationError(#[from] re_types_core::DeserializationError),

    #[error("Error serializing: {0}")]
    SerializationError(#[from] re_types_core::SerializationError),

    #[error("Error converting arrow data: {0}")]
    ArrowError(#[from] arrow2::error::Error),

    #[error("Not implemented")]
    NotImplemented,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, QueryError>;
