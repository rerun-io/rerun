//! [`CatalogToolError`]: why a catalog tool call failed.

use std::time::Duration;

use datafusion::common::TableReference;
use datafusion::error::DataFusionError;
use re_log_types::{EntityPath, InvalidTimelineNameError, TimelineName};
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

    #[error("Invalid timeline name: {source}")]
    InvalidTimeline { source: InvalidTimelineNameError },

    #[error(
        "Pass `index`, the timeline to index rows by: one of {}.",
        names(timelines)
    )]
    AmbiguousIndex { timelines: Vec<TimelineName> },

    #[error(
        "The dataset has no timeline `{index}`. Its timelines: {}.",
        names(timelines)
    )]
    NoSuchIndex {
        index: TimelineName,
        timelines: Vec<TimelineName>,
    },

    #[error("`segments` is empty. Leave it out to query every segment.")]
    NoSegments,

    #[error("`entity_paths` is empty. Leave it out to query every entity.")]
    NoEntities,

    #[error(
        "The query returns {num_columns} columns, too many to list in a response. Select fewer \
         columns."
    )]
    TooManyColumns { num_columns: usize },

    #[error("Invalid query: {source}")]
    InvalidQuery { source: DataFusionError },

    #[error(
        "Invalid query: quote table names that contain dots, e.g. `{:?}`.",
        reference.to_string()
    )]
    DottedTableName { reference: TableReference },

    #[error("The query names {num_tables} tables, but may name at most {max_tables}.")]
    TooManyTables {
        num_tables: usize,
        max_tables: usize,
    },

    #[error("Failed to open the table `{table}`: {source}")]
    OpenTable {
        table: String,
        source: DataFusionError,
    },

    #[error("Failed to set up the query: {source}")]
    SetUpQuery { source: DataFusionError },

    #[error("The query failed: {source}")]
    QueryFailed { source: DataFusionError },

    #[error(
        "The query did not finish within {} s. {hint}",
        timeout.as_secs()
    )]
    QueryTimedOut {
        timeout: Duration,

        /// What the agent can do about it, for the tool that timed out.
        hint: &'static str,
    },

    #[error("Failed to encode the result as JSON: {source}")]
    EncodeJson { source: arrow::error::ArrowError },

    #[error("Failed to read back the result as JSON: {source}")]
    DecodeJson { source: serde_json::Error },
}

/// `names`, comma-separated, or `none`.
fn names(names: &[TimelineName]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
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
