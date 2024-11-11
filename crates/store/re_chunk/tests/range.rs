use arrow2::datatypes::DataType as ArrowDatatype;
use nohash_hasher::IntMap;

use re_chunk::{Chunk, ComponentName, RangeQuery, RowId, TimePoint, Timeline};
use re_log_types::{
    example_components::{MyColor, MyLabel, MyPoint},
    ResolvedTimeRange,
};
use re_types_core::{Component as _, Loggable as _};

// ---

const ENTITY_PATH: &str = "my/entity";

fn datatypes() -> IntMap<ComponentName, ArrowDatatype> {
    [
        (MyPoint::name(), MyPoint::arrow_datatype()),
        (MyColor::name(), MyColor::arrow_datatype()),
        (MyLabel::name(), MyLabel::arrow_datatype()),
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
        let query = RangeQuery::with_extras(
            Timeline::new_sequence("frame"),
            ResolvedTimeRange::EVERYTHING,
        );

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

    {
        let query =
            RangeQuery::with_extras(Timeline::log_time(), ResolvedTimeRange::new(1020, 1050));

        let expected = chunk.emptied();
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
        let query = RangeQuery::with_extras(Timeline::log_time(), ResolvedTimeRange::EVERYTHING);

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

    {
        let query =
            RangeQuery::with_extras(Timeline::log_time(), ResolvedTimeRange::new(1020, 1050));

        let expected = chunk.emptied();
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

    let queries = [
        RangeQuery::with_extras(
            Timeline::new_sequence("frame"),
            ResolvedTimeRange::EVERYTHING,
        ),
        RangeQuery::with_extras(Timeline::log_time(), ResolvedTimeRange::new(1020, 1050)),
    ];

    for query in queries {
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

    let queries = [
        RangeQuery::with_extras(
            Timeline::new_sequence("frame"),
            ResolvedTimeRange::EVERYTHING,
        ),
        RangeQuery::with_extras(Timeline::log_time(), ResolvedTimeRange::new(1020, 1050)),
    ];

    for query in queries {
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
    (component_name, query): (ComponentName, &RangeQuery),
    chunk: &Chunk,
    expected: &Chunk,
) {
    re_log::setup_logging();

    let results = chunk.range(query, component_name);

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
