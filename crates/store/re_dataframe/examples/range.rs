use itertools::Itertools as _;

use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ComponentColumnDescriptor, RangeQueryExpression, Timeline,
    VersionPolicy,
};
use re_dataframe::QueryEngine;
use re_log_types::{ResolvedTimeRange, StoreKind};

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect_vec();

    let Some(path_to_rrd) = args.get(1) else {
        eprintln!(
            "Usage: {} <path_to_rrd>",
            args.first().map_or("$BIN", |s| s.as_str())
        );
        std::process::exit(1);
    };

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

        {
            let query = RangeQueryExpression {
                entity_path_expr: "/**".into(),
                timeline: Timeline::log_tick(),
                time_range: ResolvedTimeRange::new(0, 30),
                pov: ComponentColumnDescriptor::new::<re_types::components::Position3D>(
                    "helix/structure/scaffolding/beads".into(),
                ),
            };

            let query_handle = engine.range(&query, None /* columns */);
            eprintln!("{query}:");
            for batch in query_handle.into_iter() {
                eprintln!("{batch}");
            }
        }
        eprintln!("---");
        {
            let query = RangeQueryExpression {
                entity_path_expr: "/helix/structure/scaffolding/**".into(),
                timeline: Timeline::log_tick(),
                time_range: ResolvedTimeRange::new(0, 30),
                pov: ComponentColumnDescriptor::new::<re_types::components::Position3D>(
                    "helix/structure/scaffolding/beads".into(),
                ),
            };

            let query_handle = engine.range(&query, None /* columns */);
            eprintln!("{query}:");
            for batch in query_handle.into_iter() {
                eprintln!("{batch}");
            }
        }
    }

    Ok(())
}
