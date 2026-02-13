use re_chunk::Chunk;
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_log_types::{AbsoluteTimeRange, TimelineName, build_frame_nr};

#[test]
fn out_of_order_timeline() {
    let chunk = Chunk::builder("my_entity")
        .with_archetype_auto_row(
            [build_frame_nr(30)],
            &MyPoints::new([MyPoint::new(1.0, 1.0)]).with_colors([MyColor(1)]),
        )
        .with_archetype_auto_row(
            [build_frame_nr(10)],
            &MyPoints::update_fields().with_colors([MyColor(2)]),
        )
        .with_archetype_auto_row(
            [build_frame_nr(20)],
            &MyPoints::new([MyPoint::new(2.0, 2.0)]),
        )
        .build()
        .unwrap();

    let timeline_frame_nr = TimelineName::new("frame_nr");
    let timeline = chunk.timelines().get(&timeline_frame_nr).unwrap();
    assert!(!timeline.is_sorted());
    assert_eq!(timeline.time_range(), AbsoluteTimeRange::new(10, 30));
    assert_eq!(
        timeline.time_range_per_component(chunk.components()),
        [
            (
                MyPoints::descriptor_points().component,
                AbsoluteTimeRange::new(20, 30)
            ),
            (
                MyPoints::descriptor_colors().component,
                AbsoluteTimeRange::new(10, 30)
            ),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn in_order_forwards_timeline() {
    let chunk = Chunk::builder("my_entity")
        .with_archetype_auto_row(
            [build_frame_nr(10)],
            &MyPoints::update_fields().with_colors([MyColor(2)]),
        )
        .with_archetype_auto_row(
            [build_frame_nr(20)],
            &MyPoints::new([MyPoint::new(2.0, 2.0)]),
        )
        .with_archetype_auto_row(
            [build_frame_nr(30)],
            &MyPoints::new([MyPoint::new(1.0, 1.0)]).with_colors([MyColor(1)]),
        )
        .build()
        .unwrap();

    let timeline_frame_nr = TimelineName::new("frame_nr");
    let timeline = chunk.timelines().get(&timeline_frame_nr).unwrap();
    assert!(timeline.is_sorted());
    assert_eq!(timeline.time_range(), AbsoluteTimeRange::new(10, 30));
    assert_eq!(
        timeline.time_range_per_component(chunk.components()),
        [
            (
                MyPoints::descriptor_points().component,
                AbsoluteTimeRange::new(20, 30)
            ),
            (
                MyPoints::descriptor_colors().component,
                AbsoluteTimeRange::new(10, 30)
            ),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn in_order_backwards_timeline() {
    let chunk = Chunk::builder("my_entity")
        .with_archetype_auto_row(
            [build_frame_nr(30)],
            &MyPoints::new([MyPoint::new(1.0, 1.0)]).with_colors([MyColor(1)]),
        )
        .with_archetype_auto_row(
            [build_frame_nr(20)],
            &MyPoints::new([MyPoint::new(2.0, 2.0)]),
        )
        .with_archetype_auto_row(
            [build_frame_nr(10)],
            &MyPoints::update_fields().with_colors([MyColor(2)]),
        )
        .build()
        .unwrap();

    let timeline_frame_nr = TimelineName::new("frame_nr");
    let timeline = chunk.timelines().get(&timeline_frame_nr).unwrap();
    assert!(!timeline.is_sorted());
    assert_eq!(timeline.time_range(), AbsoluteTimeRange::new(10, 30));
    assert_eq!(
        timeline.time_range_per_component(chunk.components()),
        [
            (
                MyPoints::descriptor_points().component,
                AbsoluteTimeRange::new(20, 30)
            ),
            (
                MyPoints::descriptor_colors().component,
                AbsoluteTimeRange::new(10, 30)
            ),
        ]
        .into_iter()
        .collect()
    );
}
