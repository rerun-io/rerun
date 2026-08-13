#![allow(clippy::iter_over_hash_type)]

//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

mod analytics;
mod batch_coalescer;
mod catalog_provider;
#[cfg(not(target_arch = "wasm32"))]
mod chunk_fetcher;
mod cpu_count;
mod dataframe_query_common;
#[cfg(not(target_arch = "wasm32"))]
mod dataframe_query_provider;
#[cfg(target_arch = "wasm32")]
mod dataframe_query_provider_wasm;
mod dataset_manifest;
mod errors;
mod grpc_streaming_provider;
#[cfg(not(target_arch = "wasm32"))]
mod local_chunk_store_provider;
mod metrics_capture;
#[cfg(not(target_arch = "wasm32"))]
mod pipeline_budget;
pub(crate) mod pushdown_expressions;
#[cfg(not(target_arch = "wasm32"))]
mod segment_chunk_manifest;
mod segment_table;
mod table_entry_provider;
mod wasm_compat;

pub(crate) use self::errors::IntoDfError;
pub(crate) use analytics::{
    ConnectionAnalytics, PendingQueryAnalytics, PendingTableQueryAnalytics,
};
pub use analytics::{TableKind, TableQueryCaller};
pub use catalog_provider::RedapCatalogProviderList;
pub use cpu_count::{available_cpus, rerun_sdk_num_cpus};
pub use dataframe_query_common::{
    DataframeClientAPI, DataframeQueryTableProvider, query_from_query_expression,
};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use dataframe_query_provider::SegmentStreamExec;
#[cfg(target_arch = "wasm32")]
pub(crate) use dataframe_query_provider_wasm::SegmentStreamExec;
pub use dataset_manifest::DatasetManifestProvider;
#[cfg(not(target_arch = "wasm32"))]
pub use local_chunk_store_provider::LocalChunkStoreTableProvider;
pub use metrics_capture::{MetricsCollector, QuerySnapshot};
pub use segment_table::SegmentTableProvider;
pub use table_entry_provider::TableEntryTableProvider;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type TraceHeaders = re_perf_telemetry::TraceHeaders;
