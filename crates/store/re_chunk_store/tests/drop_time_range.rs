// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use std::sync::Arc;

use re_chunk::{Chunk, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreConfig};
use re_log_types::{example_components::MyColor, ResolvedTimeRange};
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_types_core::Component as _;

#[test]
fn drop_time_range() -> anyhow::Result<()> {
    re_log::setup_logging();

    let entity_path = EntityPath::from("this/that");
    let timeline = Timeline::new_sequence("timeline");
    let data = MyColor::from_rgb(255, 0, 0);
    let time_point_at = |time: i64| TimePoint::from([(timeline, time)]);

    for config in [
        ChunkStoreConfig::DEFAULT,
        ChunkStoreConfig::COMPACTION_DISABLED,
    ] {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            config,
        );

        let num_events = |store: &ChunkStore| {
            store.num_temporal_events_for_component_on_timeline(
                timeline.name(),
                &entity_path,
                &MyColor::descriptor(),
            )
        };

        store.insert_chunk(&Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batch(RowId::new(), time_point_at(0), &data)
                .with_component_batch(RowId::new(), time_point_at(1), &data)
                .with_component_batch(RowId::new(), time_point_at(2), &data)
                .with_component_batch(RowId::new(), time_point_at(3), &data)
                .build()?,
        ))?;

        store.insert_chunk(&Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batch(RowId::new(), time_point_at(2), &data)
                .with_component_batch(RowId::new(), time_point_at(3), &data)
                .with_component_batch(RowId::new(), time_point_at(4), &data)
                .with_component_batch(RowId::new(), time_point_at(5), &data)
                .build()?,
        ))?;

        store.insert_chunk(&Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batch(RowId::new(), time_point_at(4), &data)
                .with_component_batch(RowId::new(), time_point_at(5), &data)
                .with_component_batch(RowId::new(), time_point_at(6), &data)
                .with_component_batch(RowId::new(), time_point_at(7), &data)
                .build()?,
        ))?;

        assert_eq!(num_events(&store), 12);

        // Drop nothing:
        store.drop_time_range(timeline.name(), ResolvedTimeRange::new(10, 100));
        store.drop_time_range(timeline.name(), ResolvedTimeRange::new(-100, -10));
        assert_eq!(num_events(&store), 12);

        // Drop stuff from the middle of the first chunk, and the start of the second:
        store.drop_time_range(timeline.name(), ResolvedTimeRange::new(1, 2));
        assert_eq!(num_events(&store), 9);

        // Drop a bunch in the middle (including all of middle chunk):
        store.drop_time_range(timeline.name(), ResolvedTimeRange::new(2, 5));
        assert_eq!(num_events(&store), 3);
    }

    Ok(())
}
