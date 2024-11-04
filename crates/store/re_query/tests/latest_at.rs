// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use std::sync::Arc;

use re_chunk::RowId;
use re_chunk_store::{
    external::re_chunk::Chunk, ChunkStore, ChunkStoreSubscriber as _, LatestAtQuery,
};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    EntityPath, TimeInt, TimePoint,
};
use re_query::QueryCache;
use re_types::{Archetype as _, ComponentBatch};

// ---

#[test]
fn simple_query() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let row_id1 = RowId::new();
    let points1 = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row_id2 = RowId::new();
    let colors2 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batch(row_id1, timepoint, &points1)
        .with_component_batch(row_id2, timepoint, &colors2)
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    let expected_compound_index = (TimeInt::new_temporal(123), row_id2);
    let expected_points = &points1;
    let expected_colors = &colors2;
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

#[test]
fn static_query() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let row_id1 = RowId::new();
    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id1, timepoint, [&points as &dyn ComponentBatch])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let row_id2 = RowId::new();
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(
            row_id2,
            TimePoint::default(),
            [&colors as &dyn ComponentBatch],
        )
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    let expected_compound_index = (TimeInt::new_temporal(123), row_id1);
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

#[test]
fn invalidation() {
    let entity_path = "point";

    let test_invalidation = |query: LatestAtQuery,
                             present_data_timepoint: TimePoint,
                             past_data_timepoint: TimePoint,
                             future_data_timepoint: TimePoint| {
        let past_timestamp = past_data_timepoint
            .get(&query.timeline())
            .copied()
            .unwrap_or(TimeInt::STATIC);
        let present_timestamp = present_data_timepoint
            .get(&query.timeline())
            .copied()
            .unwrap_or(TimeInt::STATIC);

        let store = ChunkStore::new_handle(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );
        let mut caches = QueryCache::new(store.clone());

        let row_id1 = RowId::new();
        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(row_id1, present_data_timepoint.clone(), [&points as _])
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let row_id2 = RowId::new();
        let colors = vec![MyColor::from_rgb(1, 2, 3)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(row_id2, present_data_timepoint.clone(), [&colors as _])
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let expected_compound_index = (present_timestamp, row_id2);
        let expected_points = &points;
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            &store.read(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // --- Modify present ---

        // Modify the PoV component
        let row_id3 = RowId::new();
        let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(row_id3, present_data_timepoint.clone(), [&points as _])
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let expected_compound_index = (present_timestamp, row_id3);
        let expected_points = &points;
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            &store.read(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let row_id4 = RowId::new();
        let colors = vec![MyColor::from_rgb(4, 5, 6), MyColor::from_rgb(7, 8, 9)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(row_id4, present_data_timepoint.clone(), [&colors as _])
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let expected_compound_index = (present_timestamp, row_id4);
        let expected_points = &points;
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            &store.read(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // --- Modify past ---

        // Modify the PoV component
        let row_id5 = RowId::new();
        let points_past = vec![MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(row_id5, past_data_timepoint.clone(), [&points_past as _])
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let expected_compound_index = (present_timestamp, row_id4);
        let expected_points = if past_timestamp.is_static() {
            &points_past
        } else {
            &points
        };
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            &store.read(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let row_id6 = RowId::new();
        let colors_past = vec![MyColor::from_rgb(10, 11, 12), MyColor::from_rgb(13, 14, 15)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(row_id6, past_data_timepoint.clone(), [&colors_past as _])
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let (expected_compound_index, expected_colors) = if past_timestamp.is_static() {
            ((past_timestamp, row_id6), &colors_past)
        } else {
            ((present_timestamp, row_id4), &colors)
        };
        query_and_compare(
            &caches,
            &store.read(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // --- Modify future ---

        // Modify the PoV component
        let row_id7 = RowId::new();
        let points_future = vec![MyPoint::new(1000.0, 2000.0), MyPoint::new(3000.0, 4000.0)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(
                row_id7,
                future_data_timepoint.clone(),
                [&points_future as _],
            )
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let (expected_compound_index, expected_points) = if past_timestamp.is_static() {
            ((past_timestamp, row_id6), &points_past)
        } else {
            ((present_timestamp, row_id4), &points)
        };
        query_and_compare(
            &caches,
            &store.read(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let row_id8 = RowId::new();
        let colors_future = vec![MyColor::from_rgb(16, 17, 18)];
        let chunk = Chunk::builder(entity_path.into())
            .with_component_batches(
                row_id8,
                future_data_timepoint.clone(),
                [&colors_future as _],
            )
            .build()
            .unwrap();
        insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

        let (expected_compound_index, expected_colors) = if past_timestamp.is_static() {
            ((past_timestamp, row_id6), &colors_past)
        } else {
            ((present_timestamp, row_id4), &colors)
        };
        query_and_compare(
            &caches,
            &store.read(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );
    };

    let static_ = TimePoint::default();
    let frame_122 = build_frame_nr(122);
    let frame_123 = build_frame_nr(123);
    let frame_124 = build_frame_nr(124);

    test_invalidation(
        LatestAtQuery::new(frame_123.0, frame_123.1),
        [frame_123].into(),
        [frame_122].into(),
        [frame_124].into(),
    );

    test_invalidation(
        LatestAtQuery::new(frame_123.0, frame_123.1),
        [frame_123].into(),
        static_,
        [frame_124].into(),
    );
}

// Test the following scenario:
// ```py
// rr.log("points", rr.Points3D([1, 2, 3]), static=True)
//
// # Do first query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[]
//
// rr.set_time(2)
// rr.log_components("points", rr.components.MyColor(0xFF0000))
//
// # Do second query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[0xFF0000]
//
// rr.set_time(3)
// rr.log_components("points", rr.components.MyColor(0x0000FF))
//
// # Do third query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[0x0000FF]
//
// rr.set_time(3)
// rr.log_components("points", rr.components.MyColor(0x00FF00))
//
// # Do fourth query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[0x00FF00]
// ```
#[test]
fn invalidation_of_future_optionals() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "points";

    let static_ = TimePoint::default();
    let frame2 = [build_frame_nr(2)];
    let frame3 = [build_frame_nr(3)];

    let query_time = [build_frame_nr(9999)];

    let row_id1 = RowId::new();
    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id1, static_, [&points as _])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row_id1);
    let expected_points = &points;
    let expected_colors = &[];
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let row_id2 = RowId::new();
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id2, frame2, [&colors as _])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::new_temporal(2), row_id2);
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let row_id3 = RowId::new();
    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id3, frame3, [&colors as _])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::new_temporal(3), row_id3);
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let row_id4 = RowId::new();
    let colors = vec![MyColor::from_rgb(0, 255, 0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id4, frame3, [&colors as _])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::new_temporal(3), row_id4);
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

#[test]
fn static_invalidation() {
    let store = ChunkStore::new_handle(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(store.clone());

    let entity_path = "points";

    let timeless = TimePoint::default();

    let query_time = [build_frame_nr(9999)];

    let row_id1 = RowId::new();
    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id1, timeless.clone(), [&points as _])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row_id1);
    let expected_points = &points;
    let expected_colors = &[];
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let row_id2 = RowId::new();
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id2, timeless.clone(), [&colors as _])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row_id2);
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let row_id3 = RowId::new();
    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let chunk = Chunk::builder(entity_path.into())
        .with_component_batches(row_id3, timeless.clone(), [&colors as _])
        .build()
        .unwrap();
    insert_and_react(&mut store.write(), &mut caches, &Arc::new(chunk));

    let query = re_chunk_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row_id3);
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        &store.read(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

// ---

fn insert_and_react(store: &mut ChunkStore, caches: &mut QueryCache, chunk: &Arc<Chunk>) {
    caches.on_events(&store.insert_chunk(chunk).unwrap());
}

fn query_and_compare(
    caches: &QueryCache,
    store: &ChunkStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    expected_compound_index: (TimeInt, RowId),
    expected_points: &[MyPoint],
    expected_colors: &[MyColor],
) {
    re_log::setup_logging();

    for _ in 0..3 {
        let cached = caches.latest_at(
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let cached_points = cached.component_batch::<MyPoint>().unwrap();
        let cached_colors = cached.component_batch::<MyColor>().unwrap_or_default();

        eprintln!("{store}");
        eprintln!("{query:?}");
        // eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(expected_compound_index, cached.compound_index);
        similar_asserts::assert_eq!(expected_points, cached_points);
        similar_asserts::assert_eq!(expected_colors, cached_colors);
    }
}
