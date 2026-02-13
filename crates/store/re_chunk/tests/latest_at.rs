use arrow::datatypes::DataType as ArrowDatatype;
use nohash_hasher::IntMap;
use re_chunk::{Chunk, LatestAtQuery, RowId, TimePoint, Timeline, TimelineName};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_types_core::{ComponentDescriptor, Loggable as _};

// ---

const ENTITY_PATH: &str = "my/entity";

fn datatypes() -> IntMap<ComponentDescriptor, ArrowDatatype> {
    [
        (MyPoints::descriptor_points(), MyPoint::arrow_datatype()),
        (MyPoints::descriptor_colors(), MyColor::arrow_datatype()),
        (MyPoints::descriptor_labels(), MyLabel::arrow_datatype()),
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
        let query = LatestAtQuery::new(TimelineName::new("frame"), 2);

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
        let query = LatestAtQuery::new(TimelineName::new("frame"), 4);

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
        let query = LatestAtQuery::new(TimelineName::new("frame"), 6);

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
        let query = LatestAtQuery::new(TimelineName::new("frame"), frame_nr);

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
    eprintln!("Expected:\n{expected}");
    eprintln!("Results:\n{results}");

    assert_eq!(
        *expected,
        results,
        "{}",
        similar_asserts::SimpleDiff::from_str(
            &format!("{results}"),
            &format!("{expected}"),
            // &format!("{results:#?}"),
            // &format!("{expected:#?}"),
            "got",
            "expected",
        ),
    );
}
