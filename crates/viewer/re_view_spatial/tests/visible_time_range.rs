//! Checks that inter- and intra-timestamp partial updates are properly handled by range queries,

#![expect(clippy::unnecessary_fallible_conversions)]

use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimeInt, TimePoint, TimeReal, Timeline};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::datatypes::VisibleTimeRange;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView2D;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn intra_timestamp_data(test_context: &mut TestContext) {
    let timeline = Timeline::new_sequence("frame");
    let points_path = EntityPath::from("points");

    let frame = |sequence: i64| {
        TimePoint::default().with(
            timeline,
            TimeInt::from_sequence(sequence.try_into().expect("unexpected min value")),
        )
    };

    // Note on positions:
    // Blueprint is configured to show range from 0 to 100 in x/y.
    // All points should fit snug in that area.

    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(42),
            &Points2D::update_fields()
                .with_positions([(20.0, 20.0), (50.0, 50.0)])
                .with_radii([3.0])
                .with_colors([0xFF0000FF]),
        )
    });

    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(43),
            &Points2D::update_fields().with_radii([5.0]),
        )
    });

    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(44),
            &Points2D::update_fields().with_colors([0x0000FFFF]),
        )
    });

    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(45),
            &Points2D::update_fields().with_positions([(20.0, 50.0), (50.0, 20.0)]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(45),
            &Points2D::update_fields().with_radii([10.0]),
        )
    });

    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(46),
            &Points2D::update_fields().with_radii([10.0]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(46),
            &Points2D::update_fields().with_positions([(20.0, 80.0), (50.0, 80.0)]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(46),
            &Points2D::update_fields().with_radii([7.0]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(46),
            &Points2D::update_fields().with_colors([0x00FF00FF]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(46),
            &Points2D::update_fields().with_positions([(80.0, 80.0), (80.0, 10.0)]),
        )
    });

    test_context.set_active_timeline(*timeline.name());
}

#[test]
fn intra_timestamp_test() {
    let mut snapshot_results = SnapshotResults::new();
    let range_absolute = |start: i64, end: i64| VisibleTimeRange {
        timeline: "frame".into(),
        range: re_sdk_types::datatypes::TimeRange {
            start: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_sequence(start.try_into().unwrap()).into(),
            ),
            end: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_sequence(end.try_into().unwrap()).into(),
            ),
        },
    };

    run_visible_time_range_test(
        "intra_timestamp/42_42",
        intra_timestamp_data,
        Some(range_absolute(42, 42)),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "intra_timestamp/43_44",
        intra_timestamp_data,
        Some(range_absolute(43, 44)),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "intra_timestamp/42_44",
        intra_timestamp_data,
        Some(range_absolute(42, 44)),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "intra_timestamp/43_45",
        intra_timestamp_data,
        Some(range_absolute(43, 45)),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "intra_timestamp/46_46",
        intra_timestamp_data,
        Some(range_absolute(46, 46)),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "intra_timestamp/infinity_infinity",
        intra_timestamp_data,
        Some(VisibleTimeRange {
            timeline: "frame".into(),
            range: re_sdk_types::datatypes::TimeRange {
                start: re_sdk_types::datatypes::TimeRangeBoundary::Infinite,
                end: re_sdk_types::datatypes::TimeRangeBoundary::Infinite,
            },
        }),
        None,
        &mut snapshot_results,
    );
}

fn visible_timerange_data(test_context: &mut TestContext) {
    let timeline = Timeline::new_duration("timestamp");
    {
        for i in 0..10 {
            let x = i as f32 * 10.0 + 5.0;
            let y_red = 40.0;
            let y_green = 60.0;
            let time_point = TimePoint::default().with(
                timeline,
                TimeInt::from_secs(i.try_into().expect("unexpected min value")),
            );

            for y in [y_green, y_red] {
                test_context.log_entity(format!("point_{:02}_{:02}", i, y as i32), |builder| {
                    builder.with_archetype(
                        RowId::new(),
                        TimePoint::default(),
                        &re_sdk_types::archetypes::Points2D::new([(x, y)])
                            .with_colors([0x555555FF])
                            .with_radii([4.0])
                            .with_draw_order(1.0),
                    )
                });
            }

            {
                let time_point = time_point.clone();
                test_context.log_entity("red", |builder| {
                    builder.with_archetype(
                        RowId::new(),
                        time_point,
                        &re_sdk_types::archetypes::Points2D::new([(x, y_red)])
                            .with_colors([0xFF0000FF])
                            .with_radii([4.0])
                            .with_draw_order(3.0),
                    )
                });
            }

            test_context.log_entity("green", |builder| {
                builder.with_archetype(
                    RowId::new(),
                    time_point,
                    &re_sdk_types::archetypes::Points2D::new([(x, y_green)])
                        .with_colors([0x00FF00FF])
                        .with_radii([4.0])
                        .with_draw_order(3.0),
                )
            });
        }
    }

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline(*timeline.name()),
            TimeControlCommand::SetTime(TimeReal::from_secs(4.5)),
        ],
    );
}

#[test]
fn test_visible_time_range_latest_at() {
    let mut snapshot_results = SnapshotResults::new();
    let range_absolute = |timeline: &str, start: i64, end: i64| VisibleTimeRange {
        timeline: timeline.into(),
        range: re_sdk_types::datatypes::TimeRange {
            start: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_millis(start.try_into().unwrap()).into(),
            ),
            end: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_millis(end.try_into().unwrap()).into(),
            ),
        },
    };

    run_visible_time_range_test(
        "visible_time_range/latest_at",
        visible_timerange_data,
        None,
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "visible_time_range/infinite",
        visible_timerange_data,
        Some(VisibleTimeRange {
            timeline: "timestamp".into(),
            range: re_sdk_types::datatypes::TimeRange {
                start: re_sdk_types::datatypes::TimeRangeBoundary::Infinite,
                end: re_sdk_types::datatypes::TimeRangeBoundary::Infinite,
            },
        }),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "visible_time_range/cursor_relative",
        visible_timerange_data,
        Some(VisibleTimeRange {
            timeline: "timestamp".into(),
            range: re_sdk_types::datatypes::TimeRange {
                start: re_sdk_types::datatypes::TimeRangeBoundary::CursorRelative(
                    TimeInt::from_millis((-1500).try_into().unwrap()).into(),
                ),
                end: re_sdk_types::datatypes::TimeRangeBoundary::CursorRelative(
                    TimeInt::from_millis(0.try_into().unwrap()).into(),
                ),
            },
        }),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "visible_time_range/absolute",
        visible_timerange_data,
        Some(range_absolute("timestamp", 1500, 3500)),
        None,
        &mut snapshot_results,
    );

    run_visible_time_range_test(
        "visible_time_range/override",
        visible_timerange_data,
        Some(range_absolute("timestamp", 1500, 3500)),
        Some(range_absolute("timestamp", 4500, 6500)),
        &mut snapshot_results,
    );
}

fn run_visible_time_range_test(
    name: &str,
    add_data: impl FnOnce(&mut TestContext),
    view_time_range: Option<VisibleTimeRange>,
    green_time_range: Option<VisibleTimeRange>,
    snapshot_results: &mut SnapshotResults,
) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    add_data(&mut test_context);

    let view_id = setup_blueprint(&mut test_context, view_time_range, green_time_range);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        name,
        egui::vec2(200.0, 200.0),
        None,
    ));
}

fn setup_blueprint(
    test_context: &mut TestContext,
    time_range: Option<VisibleTimeRange>,
    green_time_range: Option<VisibleTimeRange>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_id = blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            SpatialView2D::identifier(),
        ));

        // Set the bounds such that the points are fully visible, that way we get more pixels contributing to the output.
        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view_id,
            ctx.store_context.blueprint.tree(),
            re_sdk_types::blueprint::archetypes::VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(
            property_path.clone(),
            &re_sdk_types::blueprint::archetypes::VisualBounds2D::new(
                re_sdk_types::datatypes::Range2D {
                    x_range: [0.0, 100.0].into(),
                    y_range: [0.0, 100.0].into(),
                },
            ),
        );

        if let Some(time_range) = time_range {
            let visible_time_range_list =
                re_sdk_types::blueprint::archetypes::VisibleTimeRanges::new([time_range]);
            let property_path = re_viewport_blueprint::entity_path_for_view_property(
                view_id,
                ctx.store_context.blueprint.tree(),
                re_sdk_types::blueprint::archetypes::VisibleTimeRanges::name(),
            );

            ctx.save_blueprint_archetype(property_path, &visible_time_range_list);
        }

        if let Some(green_time_range) = green_time_range {
            let visible_time_range_list =
                re_sdk_types::blueprint::archetypes::VisibleTimeRanges::new([green_time_range]);
            ctx.save_blueprint_archetype(
                re_viewport_blueprint::ViewContents::base_override_path_for_entity(
                    view_id,
                    &"green".into(),
                ),
                &visible_time_range_list,
            );
        }

        view_id
    })
}
