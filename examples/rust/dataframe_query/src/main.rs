//! Demonstrates basic usage of the dataframe APIs.

use itertools::Itertools;

use rerun::{
    dataframe::{
        concatenate_record_batches, EntityPathFilter, QueryCache, QueryEngine, QueryExpression,
        SparseFillStrategy, Timeline,
    },
    ChunkStore, ChunkStoreConfig, StoreKind, VersionPolicy,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect_vec();

    let get_arg = |i| {
        let Some(value) = args.get(i) else {
            let bin_name = args.first().map_or("$BIN", |s| s.as_str());
            eprintln!(
                "{}",
                unindent::unindent(&format!(
                    "\
                    Usage: {bin_name} <path_to_rrd> [entity_path_filter]

                    This example will query for the first 10 rows of data in your recording of choice,
                    and display the results as a table in your terminal.

                    You can use one of your recordings, or grab one from our hosted examples, e.g.:
                    curl 'https://app.rerun.io/version/latest/examples/dna.rrd' -o - > /tmp/dna.rrd

                    The results can be filtered further by specifying an entity filter expression:
                    {bin_name} my_recording.rrd /helix/structure/**\
                    ",
                )),
            );
            std::process::exit(1);
        };
        value
    };

    let path_to_rrd = get_arg(1);
    let entity_path_filter = EntityPathFilter::try_from(args.get(2).map_or("/**", |s| s.as_str()))?;
    let timeline = Timeline::log_time();

    let stores = ChunkStore::from_rrd_filepath(
        &ChunkStoreConfig::DEFAULT,
        path_to_rrd,
        VersionPolicy::Warn,
    )?;

    for (store_id, store) in &stores {
        if store_id.kind != StoreKind::Recording {
            continue;
        }

        let query_cache = QueryCache::new(store);
        let query_engine = QueryEngine {
            store,
            cache: &query_cache,
        };

        let query = QueryExpression {
            filtered_index: Some(timeline),
            view_contents: Some(
                query_engine
                    .iter_entity_paths(&entity_path_filter)
                    .map(|entity_path| (entity_path, None))
                    .collect(),
            ),
            sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
            ..Default::default()
        };

        let query_handle = query_engine.query(query.clone());
        let record_batches = query_handle.batch_iter().take(10).collect_vec();

        let table = concatenate_record_batches(query_handle.schema().clone(), &record_batches)?;
        println!("{table}");
    }

    Ok(())
}
