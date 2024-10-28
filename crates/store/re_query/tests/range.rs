// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use std::sync::Arc;

use itertools::Itertools;

use re_chunk::{RowId, Timeline};
use re_chunk_store::{
    external::re_chunk::Chunk, ChunkStore, ChunkStoreSubscriber as _, RangeQuery,
    ResolvedTimeRange, TimeInt,
};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    EntityPath, TimePoint,
};
use re_query::QueryCache;
use re_types::Archetype;
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_range() -> anyhow::Result<()> {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(&store);

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    let row_id1_1 = RowId::new();
    let points1_1 = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row_id1_2 = RowId::new();
    let colors1_2 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id1_1, timepoint1, &points1_1)
        .with_component_batch(row_id1_2, timepoint1, &colors1_2)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));

    let timepoint2 = [build_frame_nr(223)];
    let row_id2 = RowId::new();
    let colors2 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id2, timepoint2, &colors2)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));

    let timepoint3 = [build_frame_nr(323)];
    let row_id3 = RowId::new();
    let points3 = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id3, timepoint3, &points3)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = RangeQuery::new(
        timepoint1[0].0,
        ResolvedTimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    let expected_points = &[
        ((TimeInt::new_temporal(323), row_id3), points3.as_slice()), //
    ];
    let expected_colors = &[
        ((TimeInt::new_temporal(223), row_id2), colors2.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path,
        expected_points,
        expected_colors,
    );

    // --- Second test: `[timepoint1, timepoint3]` ---

    let query = RangeQuery::new(
        timepoint1[0].0,
        ResolvedTimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let expected_points = &[
        (
            (TimeInt::new_temporal(123), row_id1_1),
            points1_1.as_slice(),
        ), //
        ((TimeInt::new_temporal(323), row_id3), points3.as_slice()), //
    ];
    let expected_colors = &[
        (
            (TimeInt::new_temporal(123), row_id1_2),
            colors1_2.as_slice(),
        ), //
        ((TimeInt::new_temporal(223), row_id2), colors2.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path,
        expected_points,
        expected_colors,
    );

    Ok(())
}

#[test]
fn static_range() -> anyhow::Result<()> {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(&store);

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    let row_id1_1 = RowId::new();
    let points1_1 = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row_id1_2 = RowId::new();
    let colors1_2 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id1_1, timepoint1, &points1_1)
        .with_component_batch(row_id1_2, timepoint1, &colors1_2)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));
    // Insert statically too!
    let row_id1_3 = RowId::new();
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id1_3, TimePoint::default(), &colors1_2)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));

    let timepoint2 = [build_frame_nr(223)];
    let row_id2_1 = RowId::new();
    let colors2_1 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id2_1, timepoint2, &colors2_1)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));
    // Insert statically too!
    let row_id2_2 = RowId::new();
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id2_2, TimePoint::default(), &colors2_1)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));

    let timepoint3 = [build_frame_nr(323)];
    // Create some Positions with implicit instances
    let row_id3 = RowId::new();
    let points3 = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(row_id3, timepoint3, &points3)
        .build()?;
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk));

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = RangeQuery::new(
        timepoint1[0].0,
        ResolvedTimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    let expected_points = &[
        ((TimeInt::new_temporal(323), row_id3), points3.as_slice()), //
    ];
    let expected_colors = &[
        ((TimeInt::STATIC, row_id2_2), colors2_1.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path,
        expected_points,
        expected_colors,
    );

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will fall back to timeless data!

    let query = RangeQuery::new(
        timepoint1[0].0,
        ResolvedTimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let expected_points = &[
        (
            (TimeInt::new_temporal(123), row_id1_1),
            points1_1.as_slice(),
        ), //
        ((TimeInt::new_temporal(323), row_id3), points3.as_slice()), //
    ];
    let expected_colors = &[
        ((TimeInt::STATIC, row_id2_2), colors2_1.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path,
        expected_points,
        expected_colors,
    );

    // --- Third test: `[-inf, +inf]` ---

    let query = RangeQuery::new(
        timepoint1[0].0,
        ResolvedTimeRange::new(TimeInt::MIN, TimeInt::MAX),
    );

    // same expectations
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path,
        expected_points,
        expected_colors,
    );

    Ok(())
}

// Test the case where the user loads a piece of data at the end of the time range, then a piece at
// the beginning of the range, and finally a piece right in the middle.
//
// DATA = ###################################################
//          |      |     |       |            \_____/
//          \______/     |       |            query #1
//          query #2     \_______/
//                       query #3
//
// There is no data invalidation involved, which is what makes this case tricky: the cache must
// properly keep track of the fact that there are holes in the data -- on purpose.
#[test]
fn time_back_and_forth() {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(&store);

    let entity_path: EntityPath = "point".into();

    let (chunks, points): (Vec<_>, Vec<_>) = (0..10)
        .map(|i| {
            let timepoint = [build_frame_nr(i)];
            let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
            let chunk = Arc::new(
                Chunk::builder(entity_path.clone())
                    .with_component_batch(RowId::new(), timepoint, &points.clone())
                    .build()
                    .unwrap(),
            );

            insert_and_react(&mut store, &mut caches, &chunk);

            (chunk, points)
        })
        .unzip();

    // --- Query #1: `[8, 10]` ---

    let query = RangeQuery::new(
        Timeline::new_sequence("frame_nr"),
        ResolvedTimeRange::new(8, 10),
    );

    let expected_points = &[
        (
            (
                TimeInt::new_temporal(8),
                chunks[8].row_id_range().unwrap().0,
            ), //
            points[8].as_slice(),
        ), //
        (
            (
                TimeInt::new_temporal(9),
                chunks[9].row_id_range().unwrap().0,
            ), //
            points[9].as_slice(),
        ), //
    ];
    query_and_compare(&caches, &store, &query, &entity_path, expected_points, &[]);

    // --- Query #2: `[1, 3]` ---

    let query = RangeQuery::new(
        Timeline::new_sequence("frame_nr"),
        ResolvedTimeRange::new(1, 3),
    );

    let expected_points = &[
        (
            (
                TimeInt::new_temporal(1),
                chunks[1].row_id_range().unwrap().0,
            ), //
            points[1].as_slice(),
        ), //
        (
            (
                TimeInt::new_temporal(2),
                chunks[2].row_id_range().unwrap().0,
            ), //
            points[2].as_slice(),
        ), //
        (
            (
                TimeInt::new_temporal(3),
                chunks[3].row_id_range().unwrap().0,
            ), //
            points[3].as_slice(),
        ), //
    ];
    query_and_compare(&caches, &store, &query, &entity_path, expected_points, &[]);

    // --- Query #3: `[5, 7]` ---

    let query = RangeQuery::new(
        Timeline::new_sequence("frame_nr"),
        ResolvedTimeRange::new(5, 7),
    );

    let expected_points = &[
        (
            (
                TimeInt::new_temporal(5),
                chunks[5].row_id_range().unwrap().0,
            ), //
            points[5].as_slice(),
        ), //
        (
            (
                TimeInt::new_temporal(6),
                chunks[6].row_id_range().unwrap().0,
            ), //
            points[6].as_slice(),
        ), //
        (
            (
                TimeInt::new_temporal(7),
                chunks[7].row_id_range().unwrap().0,
            ), //
            points[7].as_slice(),
        ), //
    ];
    query_and_compare(&caches, &store, &query, &entity_path, expected_points, &[]);
}

#[test]
fn invalidation() {
    let entity_path = "point";

    let test_invalidation = |query: RangeQuery,
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
        let future_timestamp = future_data_timepoint
            .get(&query.timeline())
            .copied()
            .unwrap_or(TimeInt::STATIC);

        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );
        let mut caches = QueryCache::new(&store);

        let row_id1 = RowId::new();
        let points1 = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let chunk1 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id1, present_data_timepoint.clone(), &points1)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk1));

        let row_id2 = RowId::new();
        let colors2 = vec![MyColor::from_rgb(1, 2, 3)];
        let chunk2 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id2, present_data_timepoint.clone(), &colors2)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk2));

        let expected_points = &[
            ((present_timestamp, row_id1), points1.as_slice()), //
        ];
        let expected_colors = &[
            ((present_timestamp, row_id2), colors2.as_slice()), //
        ];
        query_and_compare(
            &caches,
            &store,
            &query,
            &entity_path.into(),
            expected_points,
            expected_colors,
        );

        // --- Modify present ---

        // Modify the PoV component
        let row_id3 = RowId::new();
        let points3 = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let chunk3 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id3, present_data_timepoint.clone(), &points3)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk3));

        let expected_points = &[
            ((present_timestamp, row_id1), points1.as_slice()), //
            ((present_timestamp, row_id3), points3.as_slice()), //
        ];
        let expected_colors = &[
            ((present_timestamp, row_id2), colors2.as_slice()), //
        ];
        query_and_compare(
            &caches,
            &store,
            &query,
            &entity_path.into(),
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let row_id4 = RowId::new();
        let colors4 = vec![MyColor::from_rgb(4, 5, 6), MyColor::from_rgb(7, 8, 9)];
        let chunk4 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id4, present_data_timepoint.clone(), &colors4)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk4));

        let expected_points = &[
            ((present_timestamp, row_id1), points1.as_slice()), //
            ((present_timestamp, row_id3), points3.as_slice()), //
        ];
        let expected_colors = &[
            ((present_timestamp, row_id2), colors2.as_slice()), //
            ((present_timestamp, row_id4), colors4.as_slice()), //
        ];
        query_and_compare(
            &caches,
            &store,
            &query,
            &entity_path.into(),
            expected_points,
            expected_colors,
        );

        // --- Modify past ---

        // Modify the PoV component
        let points5 = vec![MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)];
        let row_id5 = RowId::new();
        let chunk5 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id5, past_data_timepoint.clone(), &points5)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk5));

        let expected_points1 = &[
            ((past_timestamp, row_id5), points5.as_slice()), //
        ] as &[_];
        let expected_points2 = &[
            ((past_timestamp, row_id5), points5.as_slice()),    //
            ((present_timestamp, row_id1), points1.as_slice()), //
            ((present_timestamp, row_id3), points3.as_slice()), //
        ] as &[_];
        let expected_points = if past_data_timepoint.is_static() {
            expected_points1
        } else {
            expected_points2
        };
        let expected_colors = &[
            ((present_timestamp, row_id2), colors2.as_slice()), //
            ((present_timestamp, row_id4), colors4.as_slice()), //
        ];
        query_and_compare(
            &caches,
            &store,
            &query,
            &entity_path.into(),
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let row_id6 = RowId::new();
        let colors6 = vec![MyColor::from_rgb(10, 11, 12), MyColor::from_rgb(13, 14, 15)];
        let chunk6 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id6, past_data_timepoint.clone(), &colors6)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk6));

        let expected_colors1 = &[
            ((past_timestamp, row_id6), colors6.as_slice()), //
        ] as &[_];
        let expected_colors2 = &[
            ((past_timestamp, row_id6), colors6.as_slice()),    //
            ((present_timestamp, row_id2), colors2.as_slice()), //
            ((present_timestamp, row_id4), colors4.as_slice()), //
        ] as &[_];
        let expected_colors = if past_data_timepoint.is_static() {
            expected_colors1
        } else {
            expected_colors2
        };
        query_and_compare(
            &caches,
            &store,
            &query,
            &entity_path.into(),
            expected_points,
            expected_colors,
        );

        // --- Modify future ---

        // Modify the PoV component
        let row_id7 = RowId::new();
        let points7 = vec![MyPoint::new(1000.0, 2000.0), MyPoint::new(3000.0, 4000.0)];
        let chunk7 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id7, future_data_timepoint.clone(), &points7)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk7));

        let expected_points1 = &[
            ((past_timestamp, row_id5), points5.as_slice()), //
        ] as &[_];
        let expected_points2 = &[
            ((past_timestamp, row_id5), points5.as_slice()),    //
            ((present_timestamp, row_id1), points1.as_slice()), //
            ((present_timestamp, row_id3), points3.as_slice()), //
            ((future_timestamp, row_id7), points7.as_slice()),  //
        ] as &[_];
        let expected_points = if past_data_timepoint.is_static() {
            expected_points1
        } else {
            expected_points2
        };
        query_and_compare(
            &caches,
            &store,
            &query,
            &entity_path.into(),
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let row_id8 = RowId::new();
        let colors8 = vec![MyColor::from_rgb(16, 17, 18)];
        let chunk8 = Chunk::builder(entity_path.into())
            .with_component_batch(row_id8, future_data_timepoint.clone(), &colors8)
            .build()
            .unwrap();
        insert_and_react(&mut store, &mut caches, &Arc::new(chunk8));

        let expected_colors1 = &[
            ((past_timestamp, row_id6), colors6.as_slice()), //
        ] as &[_];
        let expected_colors2 = &[
            ((past_timestamp, row_id6), colors6.as_slice()),    //
            ((present_timestamp, row_id2), colors2.as_slice()), //
            ((present_timestamp, row_id4), colors4.as_slice()), //
            ((future_timestamp, row_id8), colors8.as_slice()),  //
        ] as &[_];
        let expected_colors = if past_data_timepoint.is_static() {
            expected_colors1
        } else {
            expected_colors2
        };
        query_and_compare(
            &caches,
            &store,
            &query,
            &entity_path.into(),
            expected_points,
            expected_colors,
        );
    };

    let timeless = TimePoint::default();
    let frame_122 = build_frame_nr(122);
    let frame_123 = build_frame_nr(123);
    let frame_124 = build_frame_nr(124);

    test_invalidation(
        RangeQuery::new(frame_123.0, ResolvedTimeRange::EVERYTHING),
        [frame_123].into(),
        [frame_122].into(),
        [frame_124].into(),
    );

    test_invalidation(
        RangeQuery::new(frame_123.0, ResolvedTimeRange::EVERYTHING),
        [frame_123].into(),
        timeless,
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
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(&store);

    let entity_path = "points";

    let timeless = TimePoint::default();
    let frame2 = [build_frame_nr(2)];
    let frame3 = [build_frame_nr(3)];

    let query = RangeQuery::new(frame2[0].0, ResolvedTimeRange::EVERYTHING);

    let row_id1 = RowId::new();
    let points1 = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let chunk1 = Chunk::builder(entity_path.into())
        .with_component_batch(row_id1, timeless, &points1)
        .build()
        .unwrap();
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk1));

    let expected_points = &[
        ((TimeInt::STATIC, row_id1), points1.as_slice()), //
    ];
    let expected_colors = &[];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path.into(),
        expected_points,
        expected_colors,
    );

    let row_id2 = RowId::new();
    let colors2 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk2 = Chunk::builder(entity_path.into())
        .with_component_batch(row_id2, frame2, &colors2)
        .build()
        .unwrap();
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk2));

    let expected_colors = &[
        ((TimeInt::new_temporal(2), row_id2), colors2.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path.into(),
        expected_points,
        expected_colors,
    );

    let row_id3 = RowId::new();
    let colors3 = vec![MyColor::from_rgb(0, 0, 255)];
    let chunk3 = Chunk::builder(entity_path.into())
        .with_component_batch(row_id3, frame3, &colors3)
        .build()
        .unwrap();
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk3));

    let expected_colors = &[
        ((TimeInt::new_temporal(2), row_id2), colors2.as_slice()), //
        ((TimeInt::new_temporal(3), row_id3), colors3.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path.into(),
        expected_points,
        expected_colors,
    );

    let row_id4 = RowId::new();
    let colors4 = vec![MyColor::from_rgb(0, 255, 0)];
    let chunk4 = Chunk::builder(entity_path.into())
        .with_component_batch(row_id4, frame3, &colors4)
        .build()
        .unwrap();
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk4));

    let expected_colors = &[
        ((TimeInt::new_temporal(2), row_id2), colors2.as_slice()), //
        ((TimeInt::new_temporal(3), row_id3), colors3.as_slice()), //
        ((TimeInt::new_temporal(3), row_id4), colors4.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path.into(),
        expected_points,
        expected_colors,
    );
}

#[test]
fn invalidation_static() {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(&store);

    let entity_path = "points";

    let timeless = TimePoint::default();

    let frame0 = [build_frame_nr(TimeInt::ZERO)];
    let query = RangeQuery::new(frame0[0].0, ResolvedTimeRange::EVERYTHING);

    let row_id1 = RowId::new();
    let points1 = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let chunk1 = Chunk::builder(entity_path.into())
        .with_component_batch(row_id1, timeless.clone(), &points1)
        .build()
        .unwrap();
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk1));

    let expected_points = &[
        ((TimeInt::STATIC, row_id1), points1.as_slice()), //
    ];
    let expected_colors = &[];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path.into(),
        expected_points,
        expected_colors,
    );

    let row_id2 = RowId::new();
    let colors2 = vec![MyColor::from_rgb(255, 0, 0)];
    let chunk2 = Chunk::builder(entity_path.into())
        .with_component_batch(row_id2, timeless.clone(), &colors2)
        .build()
        .unwrap();
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk2));

    let expected_colors = &[
        ((TimeInt::STATIC, row_id2), colors2.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path.into(),
        expected_points,
        expected_colors,
    );

    let row_id3 = RowId::new();
    let colors3 = vec![MyColor::from_rgb(0, 0, 255)];
    let chunk3 = Chunk::builder(entity_path.into())
        .with_component_batch(row_id3, timeless, &colors3)
        .build()
        .unwrap();
    insert_and_react(&mut store, &mut caches, &Arc::new(chunk3));

    let expected_colors = &[
        ((TimeInt::STATIC, row_id3), colors3.as_slice()), //
    ];
    query_and_compare(
        &caches,
        &store,
        &query,
        &entity_path.into(),
        expected_points,
        expected_colors,
    );
}

// See <https://github.com/rerun-io/rerun/pull/6214>.
#[test]
fn concurrent_multitenant_edge_case() {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(&store);

    let entity_path: EntityPath = "point".into();

    let add_points = |time: i64, point_value: f32| {
        let timepoint = [build_frame_nr(time)];
        let points = vec![
            MyPoint::new(point_value, point_value + 1.0),
            MyPoint::new(point_value + 2.0, point_value + 3.0),
        ];
        let chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batch(RowId::new(), timepoint, &points)
                .build()
                .unwrap(),
        );
        (timepoint, points, chunk)
    };

    let (timepoint1, points1, chunk1) = add_points(123, 1.0);
    insert_and_react(&mut store, &mut caches, &chunk1);
    let (_timepoint2, points2, chunk2) = add_points(223, 2.0);
    insert_and_react(&mut store, &mut caches, &chunk2);
    let (_timepoint3, points3, chunk3) = add_points(323, 3.0);
    insert_and_react(&mut store, &mut caches, &chunk3);

    // --- Tenant #1 queries the data, but doesn't cache the result in the deserialization cache ---

    let query = RangeQuery::new(timepoint1[0].0, ResolvedTimeRange::EVERYTHING);

    eprintln!("{store}");

    {
        let cached = caches.range(
            &store,
            &query,
            &entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let _cached_all_points = cached.get_required(&MyPoint::name()).unwrap();
    }

    // --- Meanwhile, tenant #2 queries and deserializes the data ---

    let query = RangeQuery::new(timepoint1[0].0, ResolvedTimeRange::EVERYTHING);

    let expected_points = &[
        (
            (TimeInt::new_temporal(123), chunk1.row_id_range().unwrap().0),
            points1.as_slice(),
        ), //
        (
            (TimeInt::new_temporal(223), chunk2.row_id_range().unwrap().0),
            points2.as_slice(),
        ), //
        (
            (TimeInt::new_temporal(323), chunk3.row_id_range().unwrap().0),
            points3.as_slice(),
        ), //
    ];
    query_and_compare(&caches, &store, &query, &entity_path, expected_points, &[]);
}

// See <https://github.com/rerun-io/rerun/issues/6279>.
#[test]
fn concurrent_multitenant_edge_case2() {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = QueryCache::new(&store);

    let entity_path: EntityPath = "point".into();

    let add_points = |time: i64, point_value: f32| {
        let timepoint = [build_frame_nr(time)];
        let points = vec![
            MyPoint::new(point_value, point_value + 1.0),
            MyPoint::new(point_value + 2.0, point_value + 3.0),
        ];
        let chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batch(RowId::new(), timepoint, &points)
                .build()
                .unwrap(),
        );
        (timepoint, points, chunk)
    };

    let (timepoint1, points1, chunk1) = add_points(123, 1.0);
    insert_and_react(&mut store, &mut caches, &chunk1);
    let (_timepoint2, points2, chunk2) = add_points(223, 2.0);
    insert_and_react(&mut store, &mut caches, &chunk2);
    let (_timepoint3, points3, chunk3) = add_points(323, 3.0);
    insert_and_react(&mut store, &mut caches, &chunk3);
    let (_timepoint4, points4, chunk4) = add_points(423, 4.0);
    insert_and_react(&mut store, &mut caches, &chunk4);
    let (_timepoint5, points5, chunk5) = add_points(523, 5.0);
    insert_and_react(&mut store, &mut caches, &chunk5);

    // --- Tenant #1 queries the data at (123, 223), but doesn't cache the result in the deserialization cache ---

    let query1 = RangeQuery::new(timepoint1[0].0, ResolvedTimeRange::new(123, 223));
    {
        let cached = caches.range(
            &store,
            &query1,
            &entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let _cached_all_points = cached.get_required(&MyPoint::name()).unwrap();
    }

    // --- Tenant #2 queries the data at (423, 523), but doesn't cache the result in the deserialization cache ---

    let query2 = RangeQuery::new(timepoint1[0].0, ResolvedTimeRange::new(423, 523));
    {
        let cached = caches.range(
            &store,
            &query2,
            &entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let _cached_all_points = cached.get_required(&MyPoint::name()).unwrap();
    }

    // --- Tenant #2 queries the data at (223, 423) and deserializes it ---

    let query3 = RangeQuery::new(timepoint1[0].0, ResolvedTimeRange::new(223, 423));
    let expected_points = &[
        (
            (TimeInt::new_temporal(223), chunk2.row_id_range().unwrap().0),
            points2.as_slice(),
        ), //
        (
            (TimeInt::new_temporal(323), chunk3.row_id_range().unwrap().0),
            points3.as_slice(),
        ), //
        (
            (TimeInt::new_temporal(423), chunk4.row_id_range().unwrap().0),
            points4.as_slice(),
        ), //
    ];
    query_and_compare(&caches, &store, &query3, &entity_path, expected_points, &[]);

    // --- Tenant #1 finally deserializes its data ---

    let expected_points = &[
        (
            (TimeInt::new_temporal(123), chunk1.row_id_range().unwrap().0),
            points1.as_slice(),
        ), //
        (
            (TimeInt::new_temporal(223), chunk2.row_id_range().unwrap().0),
            points2.as_slice(),
        ), //
    ];
    query_and_compare(&caches, &store, &query1, &entity_path, expected_points, &[]);

    // --- Tenant #2 finally deserializes its data ---

    let expected_points = &[
        (
            (TimeInt::new_temporal(423), chunk4.row_id_range().unwrap().0),
            points4.as_slice(),
        ), //
        (
            (TimeInt::new_temporal(523), chunk5.row_id_range().unwrap().0),
            points5.as_slice(),
        ), //
    ];
    query_and_compare(&caches, &store, &query2, &entity_path, expected_points, &[]);
}

// // ---

fn insert_and_react(store: &mut ChunkStore, caches: &mut QueryCache, chunk: &Arc<Chunk>) {
    caches.on_events(&store.insert_chunk(chunk).unwrap());
}

fn query_and_compare(
    caches: &QueryCache,
    store: &ChunkStore,
    query: &RangeQuery,
    entity_path: &EntityPath,
    expected_all_points_indexed: &[((TimeInt, RowId), &[MyPoint])],
    expected_all_colors_indexed: &[((TimeInt, RowId), &[MyColor])],
) {
    re_log::setup_logging();

    for _ in 0..3 {
        let cached = caches.range(
            store,
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let all_points_chunks = cached.get_required(&MyPoint::name()).unwrap();
        let all_points_indexed = all_points_chunks
            .iter()
            .flat_map(|chunk| {
                itertools::izip!(
                    chunk.iter_component_indices(&query.timeline(), &MyPoint::name()),
                    chunk.iter_component::<MyPoint>()
                )
            })
            .collect_vec();
        // Only way I've managed to make `rustc` realize there's a `PartialEq` available.
        let all_points_indexed = all_points_indexed
            .iter()
            .map(|(index, points)| (*index, points.as_slice()))
            .collect_vec();

        let all_colors_chunks = cached.get(&MyColor::name()).unwrap_or_default();
        let all_colors_indexed = all_colors_chunks
            .iter()
            .flat_map(|chunk| {
                itertools::izip!(
                    chunk.iter_component_indices(&query.timeline(), &MyColor::name()),
                    chunk.iter_primitive::<u32>(&MyColor::name()),
                )
            })
            .collect_vec();
        // Only way I've managed to make `rustc` realize there's a `PartialEq` available.
        let all_colors_indexed = all_colors_indexed
            .iter()
            .map(|(index, colors)| (*index, bytemuck::cast_slice(colors)))
            .collect_vec();

        eprintln!("{query:?}");
        eprintln!("{store}");

        similar_asserts::assert_eq!(expected_all_points_indexed, all_points_indexed);
        similar_asserts::assert_eq!(expected_all_colors_indexed, all_colors_indexed);
    }
}
