use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimePoint};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, TimeInt};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint},
    EntityPath, Timeline,
};
use re_types_core::Component as _;

#[test]
fn stats() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    {
        let chunk = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::new_temporal(0))],
                [
                    (MyColor::descriptor(), None),
                    (MyPoint::descriptor(), Some(&MyPoint::from_iter(0..1) as _)),
                ],
            )
            .with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::new_temporal(1))],
                [
                    (MyColor::descriptor(), Some(&MyColor::from_iter(2..3) as _)),
                    (MyPoint::descriptor(), None),
                ],
            )
            .with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::new_temporal(2))],
                [
                    (MyColor::descriptor(), Some(&MyColor::from_iter(2..3) as _)),
                    (MyPoint::descriptor(), Some(&MyPoint::from_iter(2..3) as _)),
                ],
            )
            .build()?;

        let chunk = Arc::new(chunk);
        eprintln!("chunk 1:\n{chunk}");
        store.insert_chunk(&chunk)?;

        assert_eq!(chunk.num_rows(), 3);
        assert_eq!(chunk.num_components(), 2);
        assert_eq!(chunk.num_events_cumulative(), 4);
    }

    {
        let chunk = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                RowId::new(),
                [build_frame_nr(TimeInt::new_temporal(3))],
                [
                    (MyColor::descriptor(), None),
                    (MyPoint::descriptor(), Some(&MyPoint::from_iter(1..2) as _)),
                ],
            )
            .build()?;

        let chunk = Arc::new(chunk);
        eprintln!("chunk 2:\n{chunk}");
        store.insert_chunk(&chunk)?;

        assert_eq!(chunk.num_rows(), 1);
        assert_eq!(
            chunk.num_components(),
            1,
            "MyColor has no data, so shouldn't count"
        );
        assert_eq!(chunk.num_events_cumulative(), 1);
    }

    {
        let chunk = Chunk::builder(entity_path.clone())
            .with_sparse_component_batches(
                RowId::new(),
                TimePoint::default(),
                [
                    (MyColor::descriptor(), None),
                    (MyPoint::descriptor(), Some(&MyPoint::from_iter(0..1) as _)),
                ],
            )
            .with_sparse_component_batches(
                RowId::new(),
                TimePoint::default(),
                [
                    (MyColor::descriptor(), Some(&MyColor::from_iter(2..3) as _)),
                    (MyPoint::descriptor(), None),
                ],
            )
            .with_sparse_component_batches(
                RowId::new(),
                TimePoint::default(),
                [
                    (MyColor::descriptor(), Some(&MyColor::from_iter(2..3) as _)),
                    (MyPoint::descriptor(), Some(&MyPoint::from_iter(2..3) as _)),
                ],
            )
            .build()?;

        let chunk = Arc::new(chunk);
        eprintln!("static chunk:\n{chunk}");
        store.insert_chunk(&chunk)?;

        assert_eq!(chunk.num_rows(), 3);
        assert_eq!(chunk.num_components(), 2);
        assert_eq!(chunk.num_events_cumulative(), 4);
    }

    println!("{store}");

    {
        let stats = store.entity_stats_static(&entity_path);
        assert_eq!(stats.num_chunks, 1, "We only logged one static chunk");
        assert_eq!(stats.num_rows, 3);
        assert_eq!(stats.num_events, 4);
    }
    {
        let stats =
            store.entity_stats_on_timeline(&entity_path, &Timeline::new_sequence("frame_nr"));
        assert_eq!(stats.num_chunks, 2, "We logged two temporal chunks");
        assert_eq!(stats.num_rows, 4);
        assert_eq!(stats.num_events, 5);
    }

    Ok(())
}
