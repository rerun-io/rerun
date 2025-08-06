//! Query and display the first 10 rows of a recording.

use rerun::{
    dataframe::{QueryEngine, QueryExpression, SparseFillStrategy, TimelineName},
    external::re_format_arrow::format_record_batch,
    ChunkStoreConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();

    let path_to_rrd = &args[1];
    let timeline = TimelineName::log_time();

    let engines = QueryEngine::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path_to_rrd)?;

    let Some((_, engine)) = engines.first_key_value() else {
        return Ok(());
    };

    let query = QueryExpression {
        filtered_index: Some(timeline),
        sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
        ..Default::default()
    };

    let query_handle = engine.query(query.clone());
    for row in query_handle.batch_iter().take(10) {
        // Each row is a `RecordBatch`, which can be easily passed around across different data ecosystems.
        println!("{}", format_record_batch(&row));
    }

    Ok(())
}
