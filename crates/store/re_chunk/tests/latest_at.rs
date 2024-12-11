use arrow2::datatypes::DataType as Arrow2Datatype;
use nohash_hasher::IntMap;

use re_chunk::{Chunk, LatestAtQuery, RowId, TimePoint, Timeline};
use re_log_types::example_components::{MyColor, MyLabel, MyPoint};
use re_types_core::{Component, ComponentDescriptor, Loggable as _};

// ---

const ENTITY_PATH: &str = "my/entity";

fn datatypes() -> IntMap<ComponentDescriptor, Arrow2Datatype> {
    [
        (MyPoint::descriptor(), MyPoint::arrow2_datatype()),
        (MyColor::descriptor(), MyColor::arrow2_datatype()),
        (MyLabel::descriptor(), MyLabel::arrow2_datatype()),
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
                    (MyPoint::descriptor(), Some(points1 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 4);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoint::descriptor(), Some(points1 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 6);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoint::descriptor(), Some(points3 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
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
                    (MyPoint::descriptor(), Some(points1 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = chunk.emptied();
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::log_time(), 1050);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoint::descriptor(), Some(points1 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
    }
    {
        let query = LatestAtQuery::new(Timeline::log_time(), 1100);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoint::descriptor(), Some(points3 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
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
                    (MyPoint::descriptor(), Some(points3 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
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
                    (MyPoint::descriptor(), Some(points3 as _)),
                    (MyColor::descriptor(), None),
                    (MyLabel::descriptor(), None),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyPoint::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyColor::descriptor(), &query), &chunk, &expected);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint.clone(),
                [
                    (MyPoint::descriptor(), None),
                    (MyColor::descriptor(), Some(colors2 as _)),
                    (MyLabel::descriptor(), Some(labels2 as _)),
                ],
            )
            .build_with_datatypes(&datatypes())?;
        query_and_compare((MyLabel::descriptor(), &query), &chunk, &expected);
    }

    Ok(())
}

// TODO: explain
#[test]
fn tagging_interim() -> anyhow::Result<()> {
    let row_id1 = RowId::new();
    let row_id2 = RowId::new();
    let row_id3 = RowId::new();
    let row_id4 = RowId::new();

    let timepoint1 = [(Timeline::new_sequence("frame"), 1)];
    let timepoint2 = [(Timeline::new_sequence("frame"), 2)];
    let timepoint3 = [(Timeline::new_sequence("frame"), 3)];
    let timepoint4 = [(Timeline::new_sequence("frame"), 4)];

    // TODO: tag those differently now
    let points1: &dyn re_types_core::ComponentBatch = &[MyPoint::new(1.0, 1.0)];
    let points2 = &[MyPoint::new(2.0, 2.0)];
    let points3: &dyn re_types_core::ComponentBatch = &[MyPoint::new(3.0, 3.0)];
    let points4 = &[MyPoint::new(4.0, 4.0)];

    let mypoint_descriptor_untagged = MyPoint::descriptor();
    let mypoint_descriptor_tagged = MyPoint::descriptor()
        .with_archetype_name("rerun.archetypes.MyPoints".into())
        .with_archetype_field_name("points".into());

    let points2 =
        re_types_core::ComponentBatch::with_descriptor(points2, mypoint_descriptor_tagged.clone());
    let points4 =
        re_types_core::ComponentBatch::with_descriptor(points4, mypoint_descriptor_tagged.clone());

    let chunk = Chunk::builder(ENTITY_PATH.into())
        .with_component_batches(row_id1, timepoint1, [points1 as _])
        .with_component_batches(row_id2, timepoint2, [&points2 as _])
        .with_component_batches(row_id3, timepoint3, [points3 as _])
        .with_component_batches(row_id4, timepoint4, [&points4 as _])
        .build()?;

    {
        let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 1);

        let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
            .with_component_batches(row_id1, timepoint1, [points1])
            .build()?;
        query_and_compare(
            (mypoint_descriptor_untagged.clone(), &query),
            &chunk,
            &expected,
        );
    }
    // {
    //     let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 2);
    //
    //     // TODO: but what about the expected descriptor then?
    //     let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
    //         .with_component_batches(row_id2, timepoint2, [points2])
    //         .build()?;
    //     query_and_compare(
    //         (mypoint_descriptor_untagged.clone(), &query),
    //         &chunk,
    //         &expected,
    //     );
    // }
    // {
    //     let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 3);
    //
    //     // TODO: but what about the expected descriptor then?
    //     let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
    //         .with_component_batches(row_id3, timepoint3, [points3])
    //         .build()?;
    //     query_and_compare(
    //         (mypoint_descriptor_untagged.clone(), &query),
    //         &chunk,
    //         &expected,
    //     );
    // }
    // {
    //     let query = LatestAtQuery::new(Timeline::new_sequence("frame"), 4);
    //
    //     // TODO: but what about the expected descriptor then?
    //     let expected = Chunk::builder_with_id(chunk.id(), ENTITY_PATH.into())
    //         .with_component_batches(row_id4, timepoint4, [&points4 as _])
    //         .build()?;
    //     query_and_compare(
    //         (mypoint_descriptor_untagged.clone(), &query),
    //         &chunk,
    //         &expected,
    //     );
    // }

    Ok(())
}

// ---

fn query_and_compare(
    (component_desc, query): (ComponentDescriptor, &LatestAtQuery),
    chunk: &Chunk,
    expected: &Chunk,
) {
    re_log::setup_logging();

    let results = chunk.latest_at(query, component_desc.component_name);

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
