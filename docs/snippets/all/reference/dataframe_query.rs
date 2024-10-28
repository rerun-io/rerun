//! Query and display the first 10 rows of a recording.

use rerun::{
    dataframe::{
        QueryCache, QueryCacheHandle, QueryEngine, QueryExpression, SparseFillStrategy, Timeline,
    },
    ChunkStore, ChunkStoreConfig, ChunkStoreHandle, VersionPolicy,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();

    let path_to_rrd = &args[1];
    let timeline = Timeline::log_time();

    let mut stores = ChunkStore::from_rrd_filepath(
        &ChunkStoreConfig::DEFAULT,
        path_to_rrd,
        VersionPolicy::Warn,
    )?
    .into_values()
    .map(ChunkStoreHandle::new);

    let Some(store) = stores.next() else {
        return Ok(());
    };

    let query_cache = QueryCacheHandle::new(QueryCache::new(store.clone()));
    let query_engine = QueryEngine {
        store,
        cache: query_cache,
    };

    let query = QueryExpression {
        filtered_index: Some(timeline),
        sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
        ..Default::default()
    };

    let query_handle = query_engine.query(query.clone());
    for row in query_handle.batch_iter().take(10) {
        // Each row is a `RecordBatch`, which can be easily passed around across different data ecosystems.
        println!("{row}");
    }

    Ok(())
}
