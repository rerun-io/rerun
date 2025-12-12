//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

mod catalog_provider;
mod dataframe_query_common;
#[cfg(not(target_arch = "wasm32"))]
mod dataframe_query_provider;
#[cfg(target_arch = "wasm32")]
mod dataframe_query_provider_wasm;
mod dataset_manifest;
mod grpc_streaming_provider;
pub(crate) mod pushdown_expressions;
mod search_provider;
mod segment_table;
mod table_entry_provider;
mod wasm_compat;

pub use catalog_provider::{DEFAULT_CATALOG_NAME, RedapCatalogProvider, get_all_catalog_names};
pub use dataframe_query_common::{DataframeQueryTableProvider, query_from_query_expression};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use dataframe_query_provider::SegmentStreamExec;
#[cfg(target_arch = "wasm32")]
pub(crate) use dataframe_query_provider_wasm::SegmentStreamExec;
pub use dataset_manifest::DatasetManifestProvider;
pub use search_provider::SearchResultsTableProvider;
pub use segment_table::SegmentTableProvider;
pub use table_entry_provider::TableEntryTableProvider;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type TraceHeaders = re_perf_telemetry::TraceHeaders;
