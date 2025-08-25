//! The Rerun public data APIs. Access `DataFusion` `TableProviders`.

mod dataframe_query_common;
#[cfg(not(target_arch = "wasm32"))]
mod dataframe_query_provider;
#[cfg(target_arch = "wasm32")]
mod dataframe_query_provider_wasm;
mod datafusion_connector;
mod grpc_streaming_provider;
mod partition_table;
mod search_provider;
mod table_entry_provider;
mod wasm_compat;

pub use dataframe_query_common::{DataframeQueryTableProvider, query_from_query_expression};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use dataframe_query_provider::PartitionStreamExec;
#[cfg(target_arch = "wasm32")]
pub(crate) use dataframe_query_provider_wasm::PartitionStreamExec;
pub use datafusion_connector::DataFusionConnector;
pub use partition_table::PartitionTableProvider;
pub use search_provider::SearchResultsTableProvider;
pub use table_entry_provider::TableEntryTableProvider;
