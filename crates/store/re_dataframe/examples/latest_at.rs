use itertools::Itertools as _;

use re_chunk::{TimeInt, Timeline};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, LatestAtQueryExpression, VersionPolicy};
use re_dataframe::QueryEngine;
use re_log_types::StoreKind;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect_vec();

    let get_arg = |i| {
        let Some(value) = args.get(i) else {
            eprintln!(
                "Usage: {} <path_to_rrd> <entity_path_expr>",
                args.first().map_or("$BIN", |s| s.as_str())
            );
            std::process::exit(1);
        };
        value
    };

    let path_to_rrd = get_arg(1);
    let entity_path_expr = args.get(2).map_or("/**", |s| s.as_str());

    let stores = ChunkStore::from_rrd_filepath(
        &ChunkStoreConfig::DEFAULT,
        path_to_rrd,
        VersionPolicy::Warn,
    )?;

    for (store_id, store) in &stores {
        if store_id.kind != StoreKind::Recording {
            continue;
        }

        let cache = re_dataframe::external::re_query::Caches::new(store);
        let engine = QueryEngine {
            store,
            cache: &cache,
        };

        let query = LatestAtQueryExpression {
            entity_path_expr: entity_path_expr.into(),
            timeline: Timeline::log_time(),
            at: TimeInt::MAX,
        };

        let query_handle = engine.latest_at(&query, None /* columns */);
        let batch = query_handle.get();

        eprintln!("{query}:\n{batch}");
    }

    Ok(())
}
