//! The Rerun public data APIs. Get dataframes back from your Rerun datastore.

mod engine;
mod query;

pub use self::engine::{QueryEngine, RecordBatch};
pub use self::query::QueryHandle;

#[doc(no_inline)]
pub use self::external::arrow2::chunk::Chunk as ArrowChunk;
#[doc(no_inline)]
pub use self::external::re_query::Caches as QueryCache;

pub mod external {
    pub use re_chunk;
    pub use re_chunk_store;
    pub use re_query;

    pub use arrow2;
}
