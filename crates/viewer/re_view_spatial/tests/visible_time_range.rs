//! Checks that inter- and intra-timestamp partial updates are properly handled by range queries,

#![expect(clippy::unnecessary_fallible_conversions)]

use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimeInt, TimePoint, TimeReal, Timeline};
use re_types::{Archetype as _, archetypes::Points2D, datatypes::VisibleTimeRange};
use re_view_spatial::SpatialView2D;
use re_viewer_context::{ViewClass as _, ViewId, test_context::TestContext};
use re_viewport::test_context_ext::TestContextExt as _;
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

    let mut time_ctrl = test_context.recording_config.time_ctrl.write();
    time_ctrl.set_timeline(timeline);
}

#[test]
fn intra_timestamp_test() {
    let range_absolute = |start: i64, end: i64| VisibleTimeRange {
        timeline: "frame".into(),
        range: re_types::datatypes::TimeRange {
            start: re_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_sequence(start.try_into().unwrap()).into(),
            ),
            end: re_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_sequence(end.try_into().unwrap()).into(),
            ),
        },
    };

    run_visible_time_range_test(
        "intra_timestamp/42_42",
        intra_timestamp_data,
        Some(range_absolute(42, 42)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/43_44",
        intra_timestamp_data,
        Some(range_absolute(43, 44)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/42_44",
        intra_timestamp_data,
        Some(range_absolute(42, 44)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/43_45",
        intra_timestamp_data,
        Some(range_absolute(43, 45)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/46_46",
        intra_timestamp_data,
        Some(range_absolute(46, 46)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/infinity_infinity",
        intra_timestamp_data,
        Some(VisibleTimeRange {
            timeline: "frame".into(),
            range: re_types::datatypes::TimeRange {
                start: re_types::datatypes::TimeRangeBoundary::Infinite,
                end: re_types::datatypes::TimeRangeBoundary::Infinite,
            },
        }),
        None,
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
                        &re_types::archetypes::Points2D::new([(x, y)])
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
                        &re_types::archetypes::Points2D::new([(x, y_red)])
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
                    &re_types::archetypes::Points2D::new([(x, y_green)])
                        .with_colors([0x00FF00FF])
                        .with_radii([4.0])
                        .with_draw_order(3.0),
                )
            });
        }
    }

    let mut time_ctrl = test_context.recording_config.time_ctrl.write();
    time_ctrl.set_timeline(timeline);
    time_ctrl.set_time(TimeReal::from_secs(4.5));
}

#[test]
fn test_visible_time_range_latest_at() {
    let range_absolute = |timeline: &str, start: i64, end: i64| VisibleTimeRange {
        timeline: timeline.into(),
        range: re_types::datatypes::TimeRange {
            start: re_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_millis(start.try_into().unwrap()).into(),
            ),
            end: re_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_millis(end.try_into().unwrap()).into(),
            ),
        },
    };

    run_visible_time_range_test(
        "visible_time_range/latest_at",
        visible_timerange_data,
        None,
        None,
    );

    run_visible_time_range_test(
        "visible_time_range/infinite",
        visible_timerange_data,
        Some(VisibleTimeRange {
            timeline: "timestamp".into(),
            range: re_types::datatypes::TimeRange {
                start: re_types::datatypes::TimeRangeBoundary::Infinite,
                end: re_types::datatypes::TimeRangeBoundary::Infinite,
            },
        }),
        None,
    );

    run_visible_time_range_test(
        "visible_time_range/cursor_relative",
        visible_timerange_data,
        Some(VisibleTimeRange {
            timeline: "timestamp".into(),
            range: re_types::datatypes::TimeRange {
                start: re_types::datatypes::TimeRangeBoundary::CursorRelative(
                    TimeInt::from_millis((-1500).try_into().unwrap()).into(),
                ),
                end: re_types::datatypes::TimeRangeBoundary::CursorRelative(
                    TimeInt::from_millis(0.try_into().unwrap()).into(),
                ),
            },
        }),
        None,
    );

    run_visible_time_range_test(
        "visible_time_range/absolute",
        visible_timerange_data,
        Some(range_absolute("timestamp", 1500, 3500)),
        None,
    );

    run_visible_time_range_test(
        "visible_time_range/override",
        visible_timerange_data,
        Some(range_absolute("timestamp", 1500, 3500)),
        Some(range_absolute("timestamp", 4500, 6500)),
    );
}

fn run_visible_time_range_test(
    name: &str,
    add_data: impl FnOnce(&mut TestContext),
    view_time_range: Option<VisibleTimeRange>,
    green_time_range: Option<VisibleTimeRange>,
) {
    let mut test_context = get_test_context();
    add_data(&mut test_context);

    let view_id = setup_blueprint(&mut test_context, view_time_range, green_time_range);
    run_view_ui_and_save_snapshot(&mut test_context, view_id, name, egui::vec2(200.0, 200.0));
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView2D>();

    test_context
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
            re_types::blueprint::archetypes::VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(
            property_path.clone(),
            &re_types::blueprint::archetypes::VisualBounds2D::new(re_types::datatypes::Range2D {
                x_range: [0.0, 100.0].into(),
                y_range: [0.0, 100.0].into(),
            }),
        );

        if let Some(time_range) = time_range {
            let visible_time_range_list =
                re_types::blueprint::archetypes::VisibleTimeRanges::new([time_range]);
            let property_path = re_viewport_blueprint::entity_path_for_view_property(
                view_id,
                ctx.store_context.blueprint.tree(),
                re_types::blueprint::archetypes::VisibleTimeRanges::name(),
            );

            ctx.save_blueprint_archetype(property_path, &visible_time_range_list);
        }

        if let Some(green_time_range) = green_time_range {
            let visible_time_range_list =
                re_types::blueprint::archetypes::VisibleTimeRanges::new([green_time_range]);
            ctx.save_blueprint_archetype(
                re_viewport_blueprint::ViewContents::override_path_for_entity(
                    view_id,
                    &"green".into(),
                ),
                &visible_time_range_list,
            );
        }

        view_id
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            test_context.run_with_single_view(ctx, view_id);
        });

    harness.run();
    harness.snapshot(name);
}
