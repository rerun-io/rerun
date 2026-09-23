use arrow::datatypes::DataType as ArrowDataType;
use nohash_hasher::IntMap;
use re_chunk::{Chunk, LatestAtQuery, RowId, TimePoint, Timeline, TimelineName};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_types_core::{ArrowDataType as _, ComponentDescriptor};

// ---

const ENTITY_PATH: &str = "my/entity";

fn datatypes() -> IntMap<ComponentDescriptor, ArrowDataType> {
    [
        (MyPoints::descriptor_points(), MyPoint::arrow_data_type()),
        (MyPoints::descriptor_colors(), MyColor::arrow_data_type()),
        (MyPoints::descriptor_labels(), MyLabel::arrow_data_type()),
    ]
    .into_iter()
    .collect()
}

#[test]
fn temporal_sorted() -> anyhow::Result<()> {
    let row_id1 = RowId::new();
    let row_id2 = RowId::new();
    let row_id3 = RowId::new();

    let timepoint1 = [
        (Timeline::log_time(), 1000),
        (Timeline::new_sequence("frame"), 1),
    ];
    let timepoint2 = [
        (Timeline::log_time(), 1032),
        (Timeline::new_sequence("frame"), 3),
    ];
    let timepoint3 = [
        (Timeline::log_time(), 1064),
        (Timeline::new_sequence("frame"), 5),
    ];

    let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
    let points3 = &[
        MyPoint::new(3.0, 3.0),
        MyPoint::new(4.0, 4.0),
        MyPoint::new(5.0, 5.0),
    ];

    let colors2 = &[MyColor::from_rgb(1, 1, 1)];

    let labels2 = &[
        MyLabel("a".into()),
        MyLabel("b".into()),
        MyLabel("c".into()),
    ];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id1,
            timepoint1,
            [(MyPoints::descriptor_points(), points1 as _)],
        )
        .with_component_batches(
            row_id2,
            timepoint2,
            [
                (MyPoints::descriptor_colors(), colors2 as _),
                (MyPoints::descriptor_labels(), labels2 as _),
            ],
        )
        .with_component_batches(
            row_id3,
            timepoint3,
            [(MyPoints::descriptor_points(), points3 as _)],
        )
        .build()?;

    {
        let query = LatestAtQuery::new(TimelineName::from("frame"), 2);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(TimelineName::from("frame"), 4);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(TimelineName::from("frame"), 6);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }

    Ok(())
}

#[test]
fn temporal_unsorted() -> anyhow::Result<()> {
    let row_id1 = RowId::new();
    let row_id2 = RowId::new();
    let row_id3 = RowId::new();

    let timepoint1 = [
        (Timeline::log_time(), 1000),
        (Timeline::new_sequence("frame"), 1),
    ];
    let timepoint2 = [
        (Timeline::log_time(), 1032),
        (Timeline::new_sequence("frame"), 3),
    ];
    let timepoint3 = [
        (Timeline::log_time(), 1064),
        (Timeline::new_sequence("frame"), 5),
    ];

    let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
    let points3 = &[
        MyPoint::new(3.0, 3.0),
        MyPoint::new(4.0, 4.0),
        MyPoint::new(5.0, 5.0),
    ];

    let colors2 = &[MyColor::from_rgb(1, 1, 1)];

    let labels2 = &[
        MyLabel("a".into()),
        MyLabel("b".into()),
        MyLabel("c".into()),
    ];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id2,
            timepoint2,
            [
                (MyPoints::descriptor_colors(), colors2 as _),
                (MyPoints::descriptor_labels(), labels2 as _),
            ],
        )
        .with_component_batches(
            row_id1,
            timepoint1,
            [(MyPoints::descriptor_points(), points1 as _)],
        )
        .with_component_batches(
            row_id3,
            timepoint3,
            [(MyPoints::descriptor_points(), points3 as _)],
        )
        .build()?;

    {
        let query = LatestAtQuery::new(TimelineName::log_time(), 1000);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(TimelineName::log_time(), 1050);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(TimelineName::log_time(), 1100);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }

    Ok(())
}

#[test]
fn static_sorted() -> anyhow::Result<()> {
    let row_id1 = RowId::new();
    let row_id2 = RowId::new();
    let row_id3 = RowId::new();

    let timepoint = TimePoint::default();

    let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
    let points3 = &[
        MyPoint::new(3.0, 3.0),
        MyPoint::new(4.0, 4.0),
        MyPoint::new(5.0, 5.0),
    ];

    let colors2 = &[MyColor::from_rgb(1, 1, 1)];

    let labels2 = &[
        MyLabel("a".into()),
        MyLabel("b".into()),
        MyLabel("c".into()),
    ];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id1,
            timepoint.clone(),
            [(MyPoints::descriptor_points(), points1 as _)],
        )
        .with_component_batches(
            row_id2,
            timepoint.clone(),
            [
                (MyPoints::descriptor_colors(), colors2 as _),
                (MyPoints::descriptor_labels(), labels2 as _),
            ],
        )
        .with_component_batches(
            row_id3,
            timepoint.clone(),
            [(MyPoints::descriptor_points(), points3 as _)],
        )
        .build()?;

    for frame_nr in [2, 4, 6] {
        let query = LatestAtQuery::new(TimelineName::from("frame"), frame_nr);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id3,
                timepoint.clone(),
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }

    Ok(())
}

#[test]
fn static_unsorted() -> anyhow::Result<()> {
    let row_id1 = RowId::new();
    let row_id2 = RowId::new();
    let row_id3 = RowId::new();

    let timepoint = TimePoint::default();

    let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
    let points3 = &[
        MyPoint::new(3.0, 3.0),
        MyPoint::new(4.0, 4.0),
        MyPoint::new(5.0, 5.0),
    ];

    let colors2 = &[MyColor::from_rgb(1, 1, 1)];

    let labels2 = &[
        MyLabel("a".into()),
        MyLabel("b".into()),
        MyLabel("c".into()),
    ];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id3,
            timepoint.clone(),
            [(MyPoints::descriptor_points(), points3 as _)],
        )
        .with_component_batches(
            row_id1,
            timepoint.clone(),
            [(MyPoints::descriptor_points(), points1 as _)],
        )
        .with_component_batches(
            row_id2,
            timepoint.clone(),
            [
                (MyPoints::descriptor_colors(), colors2 as _),
                (MyPoints::descriptor_labels(), labels2 as _),
            ],
        )
        .build()?;

    for log_time in [1000, 1050, 1100] {
        let query = LatestAtQuery::new(TimelineName::log_time(), log_time);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id3,
                timepoint.clone(),
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_points(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_colors(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH)
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors2 as _)),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoints::descriptor_labels(), &query), &chunk, &expected);
    }

    Ok(())
}

// ---

// TODO(andreas): This doesn't have to take a full descriptor, but all our access methods are using descriptors right now.
fn query_and_compare(
    (component_desc, query): (ComponentDescriptor, &LatestAtQuery),
    chunk: &Chunk,
    expected: &Chunk,
) {
    re_log::setup_logging();

    let results = chunk.latest_at(query, component_desc.component);

    eprintln!("Query: {component_desc} @ {query:?}");
    eprintln!("Data:\n{chunk}");

    if expected.is_empty() {
        assert!(results.is_none(), "Expected no results, but got some");
    } else {
        let results = results.expect("Expected latest_at to return a result");
        eprintln!("Expected:\n{expected}");
        eprintln!("Results:\n{results}");

        assert_eq!(
            expected,
            &*results,
            "{}",
            similar_asserts::SimpleDiff::from_str(
                &format!("{results}"),
                &format!("{expected}"),
                "got",
                "expected",
            ),
        );
    }
}

/// Query `latest_at` and return the `(frame_time, row_id)` of the selected row, or `None`.
fn latest_at_row(chunk: &Chunk, component: &ComponentDescriptor, at: i64) -> Option<(i64, RowId)> {
    let timeline = TimelineName::from("frame");
    let query = LatestAtQuery::new(timeline, at);
    let unit = chunk.latest_at(&query, component.component)?;
    let (time, row_id) = unit
        .index(Some(&timeline))
        .expect("result must carry the frame index");
    Some((time.as_i64(), row_id))
}

/// A query before every row must answer nothing, rather than the first row in the chunk.
///
/// The time-sorted path floors its search at row zero, so an unguarded walk backwards from there
/// hands back data from the future.
#[test]
fn temporal_sorted_before_all_data() -> anyhow::Result<()> {
    re_log::setup_logging();

    let row_id1 = RowId::new();
    let row_id2 = RowId::new();

    let points1 = &[MyPoint::new(1.0, 1.0)];
    let points2 = &[MyPoint::new(2.0, 2.0)];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id1,
            [(Timeline::new_sequence("frame"), 10)],
            [(MyPoints::descriptor_points(), points1 as _)],
        )
        .with_component_batches(
            row_id2,
            [(Timeline::new_sequence("frame"), 20)],
            [(MyPoints::descriptor_points(), points2 as _)],
        )
        .build()?;

    let points = MyPoints::descriptor_points();

    assert_eq!(
        latest_at_row(&chunk, &points, 9),
        None,
        "nothing is logged at-or-before frame 9"
    );
    assert_eq!(
        latest_at_row(&chunk, &points, i64::MIN),
        None,
        "nor at the very start of time"
    );

    // The rows themselves are still reachable, so this is not an off-by-one the other way.
    assert_eq!(latest_at_row(&chunk, &points, 10), Some((10, row_id1)));
    assert_eq!(latest_at_row(&chunk, &points, 19), Some((10, row_id1)));
    assert_eq!(latest_at_row(&chunk, &points, 20), Some((20, row_id2)));

    Ok(())
}

/// `RowId` breaks ties on the query time, and a time-sorted chunk says nothing about the `RowId`
/// order within one time — so the tie-break cannot lean on row order.
#[test]
fn temporal_sorted_tie_break_row_ids_unsorted() -> anyhow::Result<()> {
    re_log::setup_logging();

    let row_id_lo = RowId::new();
    let row_id_hi = RowId::new();
    assert!(row_id_hi > row_id_lo);

    let colors_lo = &[MyColor::from_rgb(1, 1, 1)];
    let colors_hi = &[MyColor::from_rgb(2, 2, 2)];

    // The higher row-id first: equal times keep the time column sorted, the row-ids are not.
    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id_hi,
            [(Timeline::new_sequence("frame"), 3)],
            [(MyPoints::descriptor_colors(), colors_hi as _)],
        )
        .with_component_batches(
            row_id_lo,
            [(Timeline::new_sequence("frame"), 3)],
            [(MyPoints::descriptor_colors(), colors_lo as _)],
        )
        .build()?;

    assert!(
        !chunk.is_row_ids_sorted(),
        "fixture must be row-id-unsorted"
    );
    assert!(
        chunk
            .timelines()
            .get(&TimelineName::from("frame"))
            .expect("fixture must carry the frame index")
            .is_sorted(),
        "fixture must be time-sorted, to reach the sorted path"
    );

    let colors = MyPoints::descriptor_colors();
    assert_eq!(
        latest_at_row(&chunk, &colors, 4),
        Some((3, row_id_hi)),
        "a tie at frame 3 must resolve to the highest row-id, whatever the row order"
    );

    Ok(())
}
