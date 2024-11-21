use arrow2::datatypes::DataType as ArrowDatatype;
use nohash_hasher::IntMap;

use re_chunk::{Chunk, ComponentName, LatestAtQuery, RowId, TimePoint, Timeline};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint};
use re_types_core::{Component, Loggable};

// ---

const ENTITY_PATH: &str = "my/entity";

fn datatypes() -> IntMap<ComponentName, ArrowDatatype> {
    [
        (MyPoint::name(), MyPoint::arrow2_datatype()),
        (MyColor::name(), MyColor::arrow2_datatype()),
        (MyLabel::name(), MyLabel::arrow2_datatype()),
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

    let chunk = Chunk::builder(ENTITY_PATH.into())
        .with_component_batches(row_id1, timepoint1, [points1 as _])
        .with_component_batches(row_id2, timepoint2, [colors2 as _, labels2 as _])
        .with_component_batches(row_id3, timepoint3, [points3 as _])
        .build()?;

    {
        let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 2);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoint::name(), Some(points1 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 4);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoint::name(), Some(points1 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 6);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoint::name(), Some(points3 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
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

    let chunk = Chunk::builder(ENTITY_PATH.into())
        .with_component_batches(row_id2, timepoint2, [colors2 as _, labels2 as _])
        .with_component_batches(row_id1, timepoint1, [points1 as _])
        .with_component_batches(row_id3, timepoint3, [points3 as _])
        .build()?;

    {
        let query = LatestAtQuery::new(Timeline::log_time(), 1000);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoint::name(), Some(points1 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::log_time(), 1050);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoint::name(), Some(points1 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::log_time(), 1100);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoint::name(), Some(points3 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
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

    let chunk = Chunk::builder(ENTITY_PATH.into())
        .with_component_batches(row_id1, timepoint.clone(), [points1 as _])
        .with_component_batches(row_id2, timepoint.clone(), [colors2 as _, labels2 as _])
        .with_component_batches(row_id3, timepoint.clone(), [points3 as _])
        .build()?;

    for frame_nr in [2, 4, 6] {
        let query = LatestAtQuery::new(Timeline::new_sequence("frame"), frame_nr);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id3,
                timepoint.clone(),
                [
                    (MyPoint::name(), Some(points3 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
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

    let chunk = Chunk::builder(ENTITY_PATH.into())
        .with_component_batches(row_id3, timepoint.clone(), [points3 as _])
        .with_component_batches(row_id1, timepoint.clone(), [points1 as _])
        .with_component_batches(row_id2, timepoint.clone(), [colors2 as _, labels2 as _])
        .build()?;

    for log_time in [1000, 1050, 1100] {
        let query = LatestAtQuery::new(Timeline::log_time(), log_time);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id3,
                timepoint.clone(),
                [
                    (MyPoint::name(), Some(points3 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::name(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors2 as _)),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::name(), &query), &chunk, &expected);
    }

    Ok(())
}

// ---

fn query_and_compare(
    (component_name, query): (ComponentName, &LatestAtQuery),
    chunk: &Chunk,
    expected: &Chunk,
) {
    re_log::setup_logging();

    let results = chunk.latest_at(query, component_name);

    eprintln!("Query: {component_name} @ {query:?}");
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
