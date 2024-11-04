//! The Rerun public data APIs. Get dataframes back from your Rerun datastore.

mod engine;
mod query;

pub use self::engine::{QueryEngine, RecordBatch};
pub use self::query::QueryHandle;

#[doc(no_inline)]
pub use self::external::arrow2::chunk::Chunk as ArrowChunk;
#[doc(no_inline)]
pub use self::external::re_chunk::{util::concatenate_record_batches, TransportChunk};
#[doc(no_inline)]
pub use self::external::re_chunk_store::{
    ChunkStoreConfig, ChunkStoreHandle, ColumnSelector, ComponentColumnSelector, Index, IndexRange,
    IndexValue, QueryExpression, SparseFillStrategy, TimeColumnSelector, VersionPolicy,
    ViewContentsSelector,
};
#[doc(no_inline)]
pub use self::external::re_log_types::{
    EntityPath, EntityPathFilter, ResolvedTimeRange, StoreKind, TimeInt, Timeline,
};
#[doc(no_inline)]
pub use self::external::re_query::{QueryCache, QueryCacheHandle, StorageEngine};

pub mod external {
    pub use re_chunk;
    pub use re_chunk_store;
    pub use re_log_types;
    pub use re_query;

    pub use arrow2;
}
