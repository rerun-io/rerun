//! [`CatalogToolError`]: why a catalog tool call failed.

use re_log_types::EntityPath;
use re_redap_client::ApiError;

/// Why a catalog tool call failed.
///
/// The messages are read by the agent, so they say what to do next where there is something to do.
#[derive(Debug, thiserror::Error)]
pub enum CatalogToolError {
    #[error("Not connected to a catalog. Call `catalog_connect` with the catalog's URL first.")]
    NotConnected,

    #[error("Unknown tool {name:?}")]
    UnknownTool { name: String },

    #[error("Invalid arguments: {source}")]
    InvalidArguments { source: serde_json::Error },

    #[error(
        "{}",
        re_error::format_with_details(format!("Invalid catalog URL: {source}"), format!("URL: {url}"))
    )]
    InvalidUrl { url: String, source: re_uri::Error },

    #[error("No dataset named {dataset:?}. Call `catalog_list_entries` for the names.")]
    NoSuchDataset { dataset: String },

    #[error(
        "The dataset has no entity `{entity_path}`. Call `catalog_get_dataset_schema` without \
         `entity_path` for the entities."
    )]
    NoSuchEntity { entity_path: EntityPath },

    #[error("The dataset has no segment {segment_id:?}. Call `catalog_list_segments` for the ids.")]
    NoSuchSegment { segment_id: String },

    #[error(
        "Did not switch to {url:?}: a later `catalog_connect` call was issued before it answered."
    )]
    ConnectSuperseded { url: String },

    #[error("Failed to {operation}: {source}")]
    Catalog {
        /// What the tool was doing, e.g. `read the dataset schema`.
        operation: &'static str,
        source: Box<ApiError>,
    },

    #[error("Unexpected dataset schema: {source}")]
    Schema { source: re_sorbet::SorbetError },

    #[error("Failed to encode the result as JSON: {source}")]
    EncodeJson { source: arrow::error::ArrowError },

    #[error("Failed to read back the result as JSON: {source}")]
    DecodeJson { source: serde_json::Error },
}

impl CatalogToolError {
    /// A failed catalog request, while the tool was doing `operation`.
    pub fn catalog(operation: &'static str, source: ApiError) -> Self {
        Self::Catalog {
            operation,
            source: Box::new(source),
        }
    }
}
