#![allow(clippy::unwrap_used, clippy::match_same_arms)]

use itertools::Itertools;

use re_chunk::TimeInt;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, QueryExpression2, Timeline, VersionPolicy};
use re_dataframe2::{QueryCache, QueryEngine};
use re_log_types::{EntityPathFilter, ResolvedTimeRange, StoreKind};

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect_vec();

    let get_arg = |i| {
        let Some(value) = args.get(i) else {
            eprintln!(
                "Usage: {} <path_to_rrd> [timeline] [from] [to] [entity_path_filter]",
                args.first().map_or("$BIN", |s| s.as_str())
            );
            std::process::exit(1);
        };
        value
    };

    let path_to_rrd = get_arg(1);
    let timeline_name = args.get(2).map_or("log_time", |s| s.as_str());
    let time_from = args.get(3).map_or(TimeInt::MIN, |s| {
        TimeInt::new_temporal(s.parse::<i64>().unwrap())
    });
    let time_to = args.get(4).map_or(TimeInt::MAX, |s| {
        TimeInt::new_temporal(s.parse::<i64>().unwrap())
    });
    let entity_path_filter = EntityPathFilter::try_from(args.get(5).map_or("/**", |s| s.as_str()))?;

    // TODO(cmc): We need to take a selector, not a Timeline.
    let timeline = match timeline_name {
        "log_time" => Timeline::new_temporal(timeline_name),
        "log_tick" => Timeline::new_sequence(timeline_name),
        "frame" => Timeline::new_sequence(timeline_name),
        "frame_nr" => Timeline::new_sequence(timeline_name),
        _ => Timeline::new_temporal(timeline_name),
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

        let query_cache = QueryCache::new(store);
        let query_engine = QueryEngine {
            store,
            cache: &query_cache,
        };

        let mut query = QueryExpression2::new(timeline);
        query.view_contents = Some(
            query_engine
                .iter_entity_paths(&entity_path_filter)
                .map(|entity_path| (entity_path, None))
                .collect(),
        );
        query.filtered_index_range = Some(ResolvedTimeRange::new(time_from, time_to));
        eprintln!("{query:#?}:");

        let query_handle = query_engine.query(query.clone());
        // eprintln!("{:#?}", query_handle.selected_contents());
        for batch in query_handle.into_batch_iter().skip(offset).take(len) {
            eprintln!("{batch}");
        }
    }

    Ok(())
}
