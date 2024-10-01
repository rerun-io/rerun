use itertools::Itertools as _;

use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ComponentColumnSelector, RangeQueryExpression, Timeline,
    VersionPolicy,
};
use re_dataframe::QueryEngine;
use re_log_types::{EntityPathFilter, ResolvedTimeRange, StoreKind};

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect_vec();

    let get_arg = |i| {
        let Some(value) = args.get(i) else {
            eprintln!(
                "Usage: {} <path_to_rrd_with_position3ds> <entity_path_pov> [entity_path_expr]",
                args.first().map_or("$BIN", |s| s.as_str())
            );
            std::process::exit(1);
        };
        value
    };

    let path_to_rrd = get_arg(1);
    let entity_path_pov = get_arg(2).as_str();
    let entity_path_filter = EntityPathFilter::try_from(args.get(3).map_or("/**", |s| s.as_str()))?;

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

        let query = RangeQueryExpression {
            entity_path_filter: entity_path_filter.clone(),
            timeline: Timeline::log_tick(),
            time_range: ResolvedTimeRange::new(0, 30),
            pov: ComponentColumnSelector::new::<re_types::components::Position3D>(
                entity_path_pov.into(),
            ),
        };

        let query_handle = engine.range(&query, None /* columns */);
        eprintln!("{query}:");
        for batch in query_handle.into_iter() {
            eprintln!("{batch}");
        }
    }

    Ok(())
}
