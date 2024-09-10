#![allow(clippy::unwrap_used)]

use itertools::Itertools as _;

use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ComponentColumnDescriptor, RangeQueryExpression, Timeline,
    VersionPolicy,
};
use re_dataframe::{QueryEngine, RecordBatch};
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
        &ChunkStoreConfig::ALL_DISABLED,
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
            pov: ComponentColumnDescriptor::new::<re_types::components::Position3D>(
                entity_path_pov.into(),
            ),
        };

        let query_handle = engine.range(&query, None /* columns */);
        println!("{query}:");
        println!(
            "num_chunks:{} num_rows:{}",
            query_handle.num_chunks(),
            query_handle.num_rows()
        );

        let row_range = 0..4;
        println!("range: {row_range:?}");
        concat_and_print(query_handle.get(row_range));

        let row_range = 2..6;
        println!("range: {row_range:?}");
        concat_and_print(query_handle.get(row_range));

        let row_range = 10..15;
        println!("range: {row_range:?}");
        concat_and_print(query_handle.get(row_range));

        let row_range = 0..15;
        println!("range: {row_range:?}");
        concat_and_print(query_handle.get(row_range));
    }

    Ok(())
}

fn concat_and_print(chunks: Vec<RecordBatch>) {
    use re_chunk::external::arrow2::{
        chunk::Chunk as ArrowChunk, compute::concatenate::concatenate,
    };

    let chunk = chunks.into_iter().reduce(|acc, chunk| RecordBatch {
        schema: chunk.schema.clone(),
        data: ArrowChunk::new(
            acc.data
                .iter()
                .zip(chunk.data.iter())
                .map(|(l, r)| concatenate(&[&**l, &**r]).unwrap())
                .collect(),
        ),
    });

    if let Some(chunk) = chunk {
        println!("{chunk}");
    } else {
        println!("<empty>");
    }
}
