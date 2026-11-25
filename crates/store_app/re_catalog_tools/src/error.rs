//! [`CatalogToolError`]: why a catalog tool call failed.

use std::time::Duration;

use datafusion::common::TableReference;
use datafusion::error::DataFusionError;
use re_log_types::{EntityPath, InvalidTimelineNameError, TimelineName};
use re_redap_client::ApiError;

/// What the agent should do about a catalog that needs a login.
const LOGIN_NEXT_STEP: &str = if cfg!(target_arch = "wasm32") {
    "Ask the user to log in from the viewer, then call `catalog_connect` again."
} else {
    "Call `catalog_login`, show the user the link it returns, then call `catalog_connect` again."
};

/// Why a catalog tool call failed.
///
/// The messages are read by the agent, so they say what to do next where there is something to do.
#[derive(Debug, thiserror::Error)]
pub enum CatalogToolError {
    #[error("Not connected to a catalog. Call `catalog_connect` with the catalog's URL first.")]
    NotConnected,

    #[error(
        "{}",
        re_error::format_with_details(
            format!("The catalog needs a login. {LOGIN_NEXT_STEP}"),
            source.to_string(),
        )
    )]
    LoginRequired { source: Box<ApiError> },

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Failed to log in: {source}")]
    Login {
        source: std::sync::Arc<re_auth::callback_server::Error>,
    },

    #[error("Unknown tool {name:?}")]
    UnknownTool { name: String },

    #[error(
        "Write access to the catalog is off. Do not try again. Tell the user what you wanted to \
         change, and that they can allow it: in the Rerun Viewer's agent panel settings, or by \
         starting `rerun mcp` without `--catalog-read-only`."
    )]
    WriteAccessOff,

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

    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    KnownServers {
        source: re_redap_client::KnownServersError,
    },

    #[error("No dataset or table named {entry:?}. Call `catalog_list_entries` for the names.")]
    NoSuchEntry { entry: String },

    #[error("A dataset or table named {name:?} already exists. Pick another name.")]
    EntryNameTaken { name: String },

    #[error("{source} These were done before it: {}. The ones after it were not.", done.join(", "))]
    StoppedPartway {
        done: Vec<String>,
        source: Box<Self>,
    },

    #[error("{name:?} is one of Rerun's own entries, which start with `__`. Leave it as it is.")]
    HiddenEntry { name: String },

    #[error("Invalid name {name:?}: {source}")]
    InvalidEntryName {
        name: String,
        source: re_log_types::InvalidEntryNameError,
    },

    #[error("{source}, got {layer:?}")]
    InvalidLayer {
        layer: String,
        source: re_types_core::InvalidLayerNameError,
    },

    #[error(
        "Invalid storage URL {url:?}: {source}. For a file on this machine, use \
         `catalog_upload_recordings`."
    )]
    InvalidStorageUrl {
        url: String,
        source: url::ParseError,
    },

    #[error("Invalid blueprint name: {source}")]
    InvalidBlueprintName {
        source: re_catalog_client::InvalidBlueprintNameError,
    },

    #[error("A blueprint named {name:?} is already registered in this dataset. Pick another name.")]
    BlueprintNameTaken { name: String },

    #[error("The dataset {dataset:?} has no blueprint dataset, so it cannot store blueprints.")]
    NoBlueprintDataset { dataset: String },

    #[error("`{field}` is empty.")]
    EmptyList { field: &'static str },

    #[error(
        "`{field}` has {count} items, but may have at most {max}. Split them over several calls."
    )]
    TooManyItems {
        field: &'static str,
        count: usize,
        max: usize,
    },

    #[cfg(not(target_arch = "wasm32"))]
    #[error("{path:?} is not the absolute path of an `.rrd` file.")]
    NotAnRrdPath { path: std::path::PathBuf },

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Failed to read {path:?}: {source}")]
    ReadFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Failed to upload {path:?}: {source}")]
    Upload {
        path: std::path::PathBuf,
        source: Box<re_catalog_client::WriteError>,
    },

    #[error(
        "This catalog server does not accept uploads. If it can read the file from storage, e.g. \
         S3, use `catalog_register_recordings` with its URL."
    )]
    UploadsUnsupported,

    #[error(
        "Failed to register the recordings: {source}{}",
        if *suggest_skip {
            ". If a segment already has this layer, pass `on_duplicate` as `skip` to leave it as \
             it is."
        } else {
            ""
        }
    )]
    RegistrationFailed {
        source: Box<ApiError>,

        /// Whether `on_duplicate` was `error`, so that `skip` is worth suggesting.
        suggest_skip: bool,
    },

    #[error(
        "The server accepted the registration, but checking on it failed: {source}. Check with \
         `catalog_list_segments`."
    )]
    RegistrationUnknown { source: Box<ApiError> },

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
