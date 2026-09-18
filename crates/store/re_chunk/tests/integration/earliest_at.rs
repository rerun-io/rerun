use re_chunk::{Chunk, EarliestAtQuery, LatestAtQuery, RowId, TimePoint, Timeline, TimelineName};
use re_log_types::TimeInt;
use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
use re_types_core::ComponentDescriptor;

// ---

const ENTITY_PATH: &str = "my/entity";

fn frame() -> TimelineName {
    TimelineName::from("frame")
}

/// Query `earliest_at` and return the `(frame_time, row_id)` of the selected row, or `None`.
fn earliest_at_row(
    chunk: &Chunk,
    component: &ComponentDescriptor,
    at: i64,
) -> Option<(i64, RowId)> {
    let query = EarliestAtQuery::new(frame(), at);
    let unit = chunk.earliest_at(&query, component.component)?;
    let (time, row_id) = unit
        .index(Some(&frame()))
        .expect("result must carry the frame index");
    Some((time.as_i64(), row_id))
}

/// Query `latest_at` and return the `(frame_time, row_id)` of the selected row, or `None`.
fn latest_at_row(chunk: &Chunk, component: &ComponentDescriptor, at: i64) -> Option<(i64, RowId)> {
    let query = LatestAtQuery::new(frame(), at);
    let unit = chunk.latest_at(&query, component.component)?;
    let (time, row_id) = unit
        .index(Some(&frame()))
        .expect("result must carry the frame index");
    Some((time.as_i64(), row_id))
}

/// A chunk with `points` at frame 1 and 5, and `colors`/`labels` at frame 3.
fn build_fixture() -> anyhow::Result<(Chunk, [RowId; 3])> {
    let row_id1 = RowId::new();
    let row_id2 = RowId::new();
    let row_id3 = RowId::new();

    let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
    let points3 = &[MyPoint::new(3.0, 3.0), MyPoint::new(4.0, 4.0)];
    let colors2 = &[MyColor::from_rgb(1, 1, 1)];
    let labels2 = &[MyLabel("a".into())];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id1,
            [(Timeline::new_sequence("frame"), 1)],
            [(MyPoints::descriptor_points(), points1 as _)],
        )
        .with_component_batches(
            row_id2,
            [(Timeline::new_sequence("frame"), 3)],
            [
                (MyPoints::descriptor_colors(), colors2 as _),
                (MyPoints::descriptor_labels(), labels2 as _),
            ],
        )
        .with_component_batches(
            row_id3,
            [(Timeline::new_sequence("frame"), 5)],
            [(MyPoints::descriptor_points(), points3 as _)],
        )
        .build()?;

    Ok((chunk, [row_id1, row_id2, row_id3]))
}

#[test]
fn temporal_sorted_backward_fill() -> anyhow::Result<()> {
    re_log::setup_logging();
    let (chunk, [row_id1, _row_id2, row_id3]) = build_fixture()?;
    let points = MyPoints::descriptor_points();
    let colors = MyPoints::descriptor_colors();

    // `points` exist at frame 1 and 5.
    for at in [0, 1] {
        assert_eq!(
            earliest_at_row(&chunk, &points, at),
            Some((1, row_id1)),
            "at={at}"
        );
    }
    for at in [2, 3, 4, 5] {
        assert_eq!(
            earliest_at_row(&chunk, &points, at),
            Some((5, row_id3)),
            "at={at}"
        );
    }
    // Nothing after frame 5.
    assert_eq!(earliest_at_row(&chunk, &points, 6), None);

    // `colors` only exist at frame 3.
    for at in [0, 1, 2, 3] {
        assert_eq!(
            earliest_at_row(&chunk, &colors, at).map(|(t, _)| t),
            Some(3),
            "at={at}"
        );
    }
    assert_eq!(earliest_at_row(&chunk, &colors, 4), None);

    Ok(())
}

/// Whether the `frame` time column is sorted, i.e. which of the two temporal paths runs.
fn is_time_sorted(chunk: &Chunk) -> bool {
    chunk
        .timelines()
        .get(&frame())
        .expect("fixture must carry the frame index")
        .is_sorted()
}

/// `RowId` is an index like any other: earliest-at looks for the lowest one, latest-at the highest.
///
/// This is the plain case, where both indices agree on the row order.
#[test]
fn tie_break_time_sorted() -> anyhow::Result<()> {
    re_log::setup_logging();

    let row_id_lo = RowId::new();
    let row_id_hi = RowId::new();
    assert!(row_id_hi > row_id_lo);

    let colors_lo = &[MyColor::from_rgb(1, 1, 1)];
    let colors_hi = &[MyColor::from_rgb(2, 2, 2)];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id_lo,
            [(Timeline::new_sequence("frame"), 3)],
            [(MyPoints::descriptor_colors(), colors_lo as _)],
        )
        .with_component_batches(
            row_id_hi,
            [(Timeline::new_sequence("frame"), 3)],
            [(MyPoints::descriptor_colors(), colors_hi as _)],
        )
        .build()?;

    assert!(chunk.is_row_ids_sorted() && is_time_sorted(&chunk));

    let colors = MyPoints::descriptor_colors();
    assert_eq!(
        earliest_at_row(&chunk, &colors, 2),
        Some((3, row_id_lo)),
        "a tie at frame 3 must resolve to the lowest row-id"
    );
    assert_eq!(
        latest_at_row(&chunk, &colors, 4),
        Some((3, row_id_hi)),
        "the latest-at twin resolves the same tie the other way"
    );

    Ok(())
}

/// A time-sorted chunk says nothing about the `RowId` order within one time, so the tie-break
/// cannot lean on row order.
#[test]
fn tie_break_time_sorted_row_ids_unsorted() -> anyhow::Result<()> {
    re_log::setup_logging();

    let row_id_lo = RowId::new();
    let row_id_hi = RowId::new();
    assert!(row_id_hi > row_id_lo);

    let colors_lo = &[MyColor::from_rgb(1, 1, 1)];
    let colors_hi = &[MyColor::from_rgb(2, 2, 2)];

    // The higher row-id first: equal times keep the column sorted, the row-ids are not.
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

    assert!(!chunk.is_row_ids_sorted() && is_time_sorted(&chunk));

    let colors = MyPoints::descriptor_colors();
    assert_eq!(
        earliest_at_row(&chunk, &colors, 2),
        Some((3, row_id_lo)),
        "a tie at frame 3 must resolve to the lowest row-id, whatever the row order"
    );

    assert_eq!(
        latest_at_row(&chunk, &colors, 4),
        Some((3, row_id_hi)),
        "the latest-at twin resolves the same tie the other way"
    );

    Ok(())
}

/// The unsorted path: reached only when the times themselves are out of order.
#[test]
fn tie_break_time_unsorted() -> anyhow::Result<()> {
    re_log::setup_logging();

    let row_id_lo = RowId::new();
    let row_id_hi = RowId::new();
    assert!(row_id_hi > row_id_lo);

    let colors_lo = &[MyColor::from_rgb(1, 1, 1)];
    let colors_hi = &[MyColor::from_rgb(2, 2, 2)];
    let colors_late = &[MyColor::from_rgb(3, 3, 3)];

    // A later frame first, so the time column is genuinely unsorted.
    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            RowId::new(),
            [(Timeline::new_sequence("frame"), 9)],
            [(MyPoints::descriptor_colors(), colors_late as _)],
        )
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

    assert!(!is_time_sorted(&chunk), "fixture must be time-unsorted");

    let colors = MyPoints::descriptor_colors();
    assert_eq!(
        earliest_at_row(&chunk, &colors, 2),
        Some((3, row_id_lo)),
        "a tie at frame 3 must resolve to the lowest row-id"
    );
    assert_eq!(
        latest_at_row(&chunk, &colors, 4),
        Some((3, row_id_hi)),
        "the latest-at twin resolves the same tie the other way"
    );

    Ok(())
}

#[test]
fn static_data() -> anyhow::Result<()> {
    re_log::setup_logging();

    let row_id = RowId::new();
    let colors = &[MyColor::from_rgb(1, 1, 1)];

    let chunk = Chunk::builder(ENTITY_PATH)
        .with_component_batches(
            row_id,
            TimePoint::default(), // static
            [(MyPoints::descriptor_colors(), colors as _)],
        )
        .build()?;

    // A static-only earliest_at returns the single static value.
    let unit = chunk.earliest_at(
        &EarliestAtQuery::new_static(),
        MyPoints::descriptor_colors().component,
    );
    assert_eq!(unit.and_then(|u| u.row_id()), Some(row_id));

    Ok(())
}

/// A static time handed to a *temporal* query must find no temporal data.
///
/// `LatestAtQuery` gets this from clamping `STATIC` down to `TimeInt::MIN`: nothing is logged
/// at-or-before the start of time. Earliest-at looks the other way, so it clamps up to
/// `TimeInt::MAX` to reach the same dead end.
#[test]
fn static_time_on_a_temporal_query_is_empty() -> anyhow::Result<()> {
    re_log::setup_logging();

    let (chunk, _row_ids) = build_fixture()?;

    assert_eq!(
        EarliestAtQuery::new(frame(), TimeInt::STATIC).at(),
        TimeInt::MAX,
        "STATIC must clamp away from the direction the query looks"
    );
    assert_eq!(
        LatestAtQuery::new(frame(), TimeInt::STATIC).at(),
        TimeInt::MIN,
        "the latest-at twin, for contrast"
    );

    // The fixture has `points` at frames 1 and 5. Clamping the wrong way would answer frame 1.
    let query = EarliestAtQuery::new(frame(), TimeInt::STATIC);
    assert!(
        chunk
            .earliest_at(&query, MyPoints::descriptor_points().component)
            .is_none(),
        "a static time must not reach the first value in the recording"
    );

    // A genuinely out-of-range time still means "from the beginning", and must keep working.
    assert_eq!(
        earliest_at_row(&chunk, &MyPoints::descriptor_points(), i64::MIN).map(|(t, _)| t),
        Some(1),
    );

    Ok(())
}

/// Static data has no temporal ordering, so `RowId` is the only index left to resolve along.
#[test]
fn static_tie_break() -> anyhow::Result<()> {
    re_log::setup_logging();

    let row_id_lo = RowId::new();
    let row_id_hi = RowId::new();
    assert!(row_id_hi > row_id_lo);

    let colors_lo = &[MyColor::from_rgb(1, 1, 1)];
    let colors_hi = &[MyColor::from_rgb(2, 2, 2)];

    let colors = MyPoints::descriptor_colors();

    // Both row-id orderings, to cover the row-sorted and row-unsorted paths.
    for (first, second) in [(row_id_lo, row_id_hi), (row_id_hi, row_id_lo)] {
        let (colors_first, colors_second) = if first == row_id_lo {
            (colors_lo, colors_hi)
        } else {
            (colors_hi, colors_lo)
        };

        let chunk = Chunk::builder(ENTITY_PATH)
            .with_component_batches(
                first,
                TimePoint::default(), // static
                [(colors.clone(), colors_first as _)],
            )
            .with_component_batches(
                second,
                TimePoint::default(), // static
                [(colors.clone(), colors_second as _)],
            )
            .build()?;

        assert_eq!(
            chunk.is_row_ids_sorted(),
            first == row_id_lo,
            "the two orderings must exercise both static paths"
        );

        let earliest = chunk.earliest_at(&EarliestAtQuery::new_static(), colors.component);
        assert_eq!(
            earliest.and_then(|u| u.row_id()),
            Some(row_id_lo),
            "static earliest-at must resolve to the lowest row-id"
        );

        let latest = chunk.latest_at(&LatestAtQuery::new_static(), colors.component);
        assert_eq!(
            latest.and_then(|u| u.row_id()),
            Some(row_id_hi),
            "static latest-at must resolve to the highest row-id"
        );
    }

    Ok(())
}

/// The ends of the representable time range, and queries that fall off the data entirely.
#[test]
fn time_range_edges() -> anyhow::Result<()> {
    re_log::setup_logging();

    let (chunk, [row_id1, _row_id2, row_id3]) = build_fixture()?;
    let points = MyPoints::descriptor_points();

    // The fixture has `points` at frames 1 and 5.
    assert_eq!(
        earliest_at_row(&chunk, &points, TimeInt::MIN.as_i64()),
        Some((1, row_id1)),
        "`MIN` means \"from the beginning\""
    );
    assert_eq!(
        earliest_at_row(&chunk, &points, i64::MIN),
        Some((1, row_id1)),
        "a time below the representable range clamps to `MIN`"
    );
    assert_eq!(
        earliest_at_row(&chunk, &points, 5),
        Some((5, row_id3)),
        "an exact hit on the last value"
    );
    assert_eq!(
        earliest_at_row(&chunk, &points, 6),
        None,
        "nothing is logged after frame 5"
    );
    assert_eq!(
        earliest_at_row(&chunk, &points, TimeInt::MAX.as_i64()),
        None,
        "`MAX` is past every temporal value"
    );
    assert_eq!(
        earliest_at_row(&chunk, &points, i64::MAX),
        None,
        "a time above the representable range clamps to `MAX`"
    );

    Ok(())
}

/// An empty chunk answers nothing, on either query.
#[test]
fn empty_chunk() -> anyhow::Result<()> {
    re_log::setup_logging();

    let chunk = Chunk::builder(ENTITY_PATH).build()?;
    let points = MyPoints::descriptor_points();

    assert!(chunk.is_empty());
    assert!(
        chunk
            .earliest_at(&EarliestAtQuery::new(frame(), 0), points.component)
            .is_none()
    );
    assert!(
        chunk
            .earliest_at(&EarliestAtQuery::new_static(), points.component)
            .is_none()
    );

    Ok(())
}
