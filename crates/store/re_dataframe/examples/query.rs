#![allow(clippy::unwrap_used, clippy::match_same_arms)]

use itertools::Itertools as _;

use re_dataframe::{
    ChunkStoreConfig, EntityPathFilter, QueryEngine, QueryExpression, ResolvedTimeRange,
    SparseFillStrategy, StoreKind, TimeInt,
};

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
    let entity_path_filter =
        EntityPathFilter::parse_strict(args.get(5).map_or("/**", |s| s.as_str()))?;

    let engines = QueryEngine::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path_to_rrd)?;

    for (store_id, engine) in &engines {
        if store_id.kind != StoreKind::Recording {
            continue;
        }

        let query = QueryExpression {
            filtered_index: Some(timeline_name.into()),
            view_contents: Some(
                engine
                    .iter_entity_paths_sorted(&entity_path_filter)
                    .map(|entity_path| (entity_path, None))
                    .collect(),
            ),
            filtered_index_range: Some(ResolvedTimeRange::new(time_from, time_to)),
            sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
            include_per_column_indexes: true,
            ..Default::default()
        };
        eprintln!("{query:#?}:");

        let query_handle = engine.query(query.clone());
        dbg!(query_handle.schema());
        std::process::exit(0);

        // eprintln!("{:#?}", query_handle.selected_contents());
        for batch in query_handle.into_batch_iter() {
            eprintln!(
                "{}",
                re_format_arrow::format_record_batch_opts(
                    &batch,
                    &re_format_arrow::RecordBatchFormatOpts {
                        transposed: true,
                        ..Default::default()
                    }
                )
            );
            std::process::exit(0);
        }
    }

    Ok(())
}
