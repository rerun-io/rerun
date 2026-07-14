//! Tests that visible time ranges work correctly for 2D and 3D spatial views.

#![expect(clippy::unnecessary_fallible_conversions)]
#![expect(clippy::unwrap_used)]

use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimeInt, TimePoint, TimeReal, Timeline};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::{Arrows3D, Boxes3D, LineStrips3D, Points2D, Points3D};
use re_sdk_types::blueprint::archetypes::{EyeControls3D, VisibleTimeRanges};
use re_sdk_types::components::Position3D;
use re_sdk_types::datatypes::{TimeRange, TimeRangeBoundary, VisibleTimeRange};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView2D;
use re_view_spatial::SpatialView3D;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

fn range_absolute_millis(start: i64, end: i64) -> VisibleTimeRange {
    VisibleTimeRange {
        timeline: "timestamp".into(),
        range: TimeRange {
            start: TimeRangeBoundary::Absolute(
                TimeInt::from_millis(start.try_into().unwrap()).into(),
            ),
            end: TimeRangeBoundary::Absolute(TimeInt::from_millis(end.try_into().unwrap()).into()),
        },
    }
}

fn range_infinite() -> VisibleTimeRange {
    VisibleTimeRange {
        timeline: "timestamp".into(),
        range: TimeRange {
            start: TimeRangeBoundary::Infinite,
            end: TimeRangeBoundary::Infinite,
        },
    }
}

fn range_cursor_relative(start_millis: i64, end_millis: i64) -> VisibleTimeRange {
    VisibleTimeRange {
        timeline: "timestamp".into(),
        range: TimeRange {
            start: TimeRangeBoundary::CursorRelative(
                TimeInt::from_millis(start_millis.try_into().unwrap()).into(),
            ),
            end: TimeRangeBoundary::CursorRelative(
                TimeInt::from_millis(end_millis.try_into().unwrap()).into(),
            ),
        },
    }
}

fn run_time_range_tests(
    prefix: &str,
    run_fn: impl Fn(&str, Option<VisibleTimeRange>, Option<VisibleTimeRange>, &mut SnapshotResults),
    snapshot_results: &mut SnapshotResults,
) {
    run_fn(&format!("{prefix}/latest_at"), None, None, snapshot_results);

    run_fn(
        &format!("{prefix}/infinite"),
        Some(range_infinite()),
        None,
        snapshot_results,
    );

    run_fn(
        &format!("{prefix}/cursor_relative"),
        Some(range_cursor_relative(-1500, 0)),
        None,
        snapshot_results,
    );

    run_fn(
        &format!("{prefix}/absolute"),
        Some(range_absolute_millis(1500, 3500)),
        None,
        snapshot_results,
    );

    run_fn(
        &format!("{prefix}/override"),
        Some(range_absolute_millis(1500, 3500)),
        Some(range_absolute_millis(4500, 6500)),
        snapshot_results,
    );
}

fn visible_timerange_data_2d(test_context: &mut TestContext) {
    let timeline = Timeline::new_duration("timestamp");
    for i in 0..10 {
        let x = i as f32 * 10.0 + 5.0;
        let y_red = 40.0;
        let y_green = 60.0;
        let time_point =
            TimePoint::default().with(timeline, TimeInt::from_secs(i.try_into().unwrap()));

        for y in [y_green, y_red] {
            test_context.log_entity(format!("point_{:02}_{:02}", i, y as i32), |builder| {
                builder.with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &Points2D::new([(x, y)])
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
                    &Points2D::new([(x, y_red)])
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
                &Points2D::new([(x, y_green)])
                    .with_colors([0x00FF00FF])
                    .with_radii([4.0])
                    .with_draw_order(3.0),
            )
        });
    }

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline(*timeline.name()),
            TimeControlCommand::SetTime(TimeReal::from_secs(4.5)),
        ],
    );
}

fn setup_blueprint_2d(
    test_context: &mut TestContext,
    time_range: Option<VisibleTimeRange>,
    green_time_range: Option<VisibleTimeRange>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_id = blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            SpatialView2D::identifier(),
        ));

        let engine = ctx.store_context.blueprint.storage_engine();
        let blueprint_tree = engine.store().entity_tree();
        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view_id,
            blueprint_tree,
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
            let property_path = re_viewport_blueprint::entity_path_for_view_property(
                view_id,
                blueprint_tree,
                VisibleTimeRanges::name(),
            );
            ctx.save_blueprint_archetype(property_path, &VisibleTimeRanges::new([time_range]));
        }

        if let Some(green_time_range) = green_time_range {
            ctx.save_blueprint_archetype(
                re_viewport_blueprint::ViewContents::base_override_path_for_entity(
                    view_id,
                    &"green".into(),
                ),
                &VisibleTimeRanges::new([green_time_range]),
            );
        }

        view_id
    })
}

fn run_test_2d(
    name: &str,
    add_data: impl FnOnce(&mut TestContext),
    view_time_range: Option<VisibleTimeRange>,
    override_time_range: Option<VisibleTimeRange>,
    snapshot_results: &mut SnapshotResults,
) {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();
    add_data(&mut test_context);

    let view_id = setup_blueprint_2d(&mut test_context, view_time_range, override_time_range);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        name,
        egui::vec2(200.0, 200.0),
        None,
    ));
}

#[test]
fn test_visible_time_range_2d() {
    let mut snapshot_results = SnapshotResults::new();
    run_time_range_tests(
        "visible_time_range_2d",
        |name, view_range, override_range, results| {
            run_test_2d(
                name,
                visible_timerange_data_2d,
                view_range,
                override_range,
                results,
            );
        },
        &mut snapshot_results,
    );
}

// Intra-timestamp 2D test (separate, uses sequence timeline).

fn intra_timestamp_data(test_context: &mut TestContext) {
    let timeline = Timeline::new_sequence("frame");
    let points_path = EntityPath::from("points");

    let frame = |sequence: i64| {
        TimePoint::default().with(
            timeline,
            TimeInt::from_sequence(sequence.try_into().unwrap()),
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
        range: TimeRange {
            start: TimeRangeBoundary::Absolute(
                TimeInt::from_sequence(start.try_into().unwrap()).into(),
            ),
            end: TimeRangeBoundary::Absolute(
                TimeInt::from_sequence(end.try_into().unwrap()).into(),
            ),
        },
    };

    run_test_2d(
        "intra_timestamp/42_42",
        intra_timestamp_data,
        Some(range_absolute(42, 42)),
        None,
        &mut snapshot_results,
    );

    run_test_2d(
        "intra_timestamp/43_44",
        intra_timestamp_data,
        Some(range_absolute(43, 44)),
        None,
        &mut snapshot_results,
    );

    run_test_2d(
        "intra_timestamp/42_44",
        intra_timestamp_data,
        Some(range_absolute(42, 44)),
        None,
        &mut snapshot_results,
    );

    run_test_2d(
        "intra_timestamp/43_45",
        intra_timestamp_data,
        Some(range_absolute(43, 45)),
        None,
        &mut snapshot_results,
    );

    run_test_2d(
        "intra_timestamp/46_46",
        intra_timestamp_data,
        Some(range_absolute(46, 46)),
        None,
        &mut snapshot_results,
    );

    run_test_2d(
        "intra_timestamp/infinity_infinity",
        intra_timestamp_data,
        Some(VisibleTimeRange {
            timeline: "frame".into(),
            range: TimeRange {
                start: TimeRangeBoundary::Infinite,
                end: TimeRangeBoundary::Infinite,
            },
        }),
        None,
        &mut snapshot_results,
    );
}

/// Logs `Points3D`, `LineStrips3D`, `Arrows3D`, and `Boxes3D` side by side over
/// 10 timesteps on a duration timeline at 1s intervals. Cursor is set to 4.5s.
fn visible_timerange_data_3d(test_context: &mut TestContext) {
    let timeline = Timeline::new_duration("timestamp");

    let points_path = EntityPath::from("points");
    let lines_path = EntityPath::from("lines");
    let arrows_path = EntityPath::from("arrows");
    let boxes_path = EntityPath::from("boxes");

    // One color per timestep so each is visually distinct.
    let colors: [u32; 10] = [
        0xFF0000FF, // red
        0xFF4400FF, //
        0xFF8800FF, // orange
        0xFFBB00FF, //
        0xFFFF00FF, // yellow
        0x88FF00FF, //
        0x00FF00FF, // green
        0x00FF88FF, //
        0x0088FFFF, //
        0x0000FFFF, // blue
    ];

    for i in 0..10i64 {
        let y = i as f32 * 2.0;
        let color = colors[i as usize];
        let time_point = TimePoint::default().with(timeline, TimeInt::from_secs(i as f64));

        let point_radius = 0.15 + i as f32 * 0.06;
        test_context.log_entity(points_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                time_point.clone(),
                &Points3D::new([(-7.5, y, 0.0)])
                    .with_radii([point_radius])
                    .with_colors([color]),
            )
        });

        let line_radius = 0.05 + i as f32 * 0.03;
        test_context.log_entity(lines_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                time_point.clone(),
                &LineStrips3D::new([vec![[-2.5_f32 - 1.0, y, 0.0], [-2.5 + 1.0, y, 0.0]]])
                    .with_radii([line_radius])
                    .with_colors([color]),
            )
        });

        let arrow_radius = 0.04 + i as f32 * 0.02;
        test_context.log_entity(arrows_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                time_point.clone(),
                &Arrows3D::from_vectors([[2.0, 0.0, 0.0]])
                    .with_origins([[2.5, y, 0.0]])
                    .with_radii([arrow_radius])
                    .with_colors([color]),
            )
        });

        let half = 0.2 + i as f32 * 0.06;
        test_context.log_entity(boxes_path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                time_point,
                &Boxes3D::from_centers_and_half_sizes([[7.5, y, 0.0]], [[half, half, half]])
                    .with_colors([color]),
            )
        });
    }

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline(*timeline.name()),
            TimeControlCommand::SetTime(TimeReal::from_secs(4.5)),
        ],
    );
}

fn setup_blueprint_3d(
    test_context: &mut TestContext,
    view_time_range: Option<VisibleTimeRange>,
    boxes_time_range: Option<VisibleTimeRange>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_id = blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            SpatialView3D::identifier(),
        ));

        let engine = ctx.store_context.blueprint.storage_engine();
        let blueprint_tree = engine.store().entity_tree();

        if let Some(time_range) = view_time_range {
            let property_path = re_viewport_blueprint::entity_path_for_view_property(
                view_id,
                blueprint_tree,
                VisibleTimeRanges::name(),
            );
            ctx.save_blueprint_archetype(property_path, &VisibleTimeRanges::new([time_range]));
        }

        if let Some(boxes_time_range) = boxes_time_range {
            ctx.save_blueprint_archetype(
                re_viewport_blueprint::ViewContents::base_override_path_for_entity(
                    view_id,
                    &"boxes".into(),
                ),
                &VisibleTimeRanges::new([boxes_time_range]),
            );
        }

        view_id
    })
}

fn run_test_3d(
    name: &str,
    view_time_range: Option<VisibleTimeRange>,
    override_time_range: Option<VisibleTimeRange>,
    snapshot_results: &mut SnapshotResults,
) {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();
    visible_timerange_data_3d(&mut test_context);

    let view_id = setup_blueprint_3d(&mut test_context, view_time_range, override_time_range);

    let size = egui::vec2(300.0, 300.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    test_context.with_blueprint_ctx(|ctx, _| {
        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(0.0, 9.0, 20.0),
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.0, 9.0, 0.0),
        );
    });
    test_context.handle_system_commands(&harness.ctx);
    harness.run();

    snapshot_results.add(harness.try_snapshot(name));
}

#[test]
fn test_visible_time_range_3d() {
    let mut snapshot_results = SnapshotResults::new();
    run_time_range_tests(
        "visible_time_range_3d",
        |name, view_range, override_range, results| {
            run_test_3d(name, view_range, override_range, results);
        },
        &mut snapshot_results,
    );
}

/// Test that sliding the time cursor with a cursor-relative range produces correct
/// results — i.e. the cache doesn't serve stale entries when the window shifts from
/// T=[2,3,4] to T=[3,4,5].
#[test]
fn test_sliding_window_3d() {
    let mut snapshot_results = SnapshotResults::new();

    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();
    visible_timerange_data_3d(&mut test_context);

    let view_id = setup_blueprint_3d(
        &mut test_context,
        Some(range_cursor_relative(-1000, 1000)),
        None,
    );

    let size = egui::vec2(300.0, 300.0);

    let setup_eye = |test_context: &TestContext, view_id: ViewId| {
        test_context.with_blueprint_ctx(|ctx, _| {
            let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
                ctx.current_blueprint(),
                ctx.blueprint_query(),
                view_id,
            );
            eye_property.save_blueprint_component(
                &ctx,
                &EyeControls3D::descriptor_position(),
                &Position3D::new(0.0, 9.0, 20.0),
            );
            eye_property.save_blueprint_component(
                &ctx,
                &EyeControls3D::descriptor_look_target(),
                &Position3D::new(0.0, 9.0, 0.0),
            );
        });
    };

    // Cursor to 5s → window covers T=[4s, 6s]
    {
        test_context.send_time_commands(
            test_context.active_store_id(),
            [TimeControlCommand::SetTime(TimeReal::from_secs(5.0))],
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_3d(size)
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id);
            });
        setup_eye(&test_context, view_id);
        test_context.handle_system_commands(&harness.ctx);
        harness.run_steps(10);
        snapshot_results.add(harness.try_snapshot("visible_time_range_3d/sliding_window_cursor_1"));
    }

    // Cursor at 3s → window covers T=[2s, 4s]
    {
        test_context.send_time_commands(
            test_context.active_store_id(),
            [TimeControlCommand::SetTime(TimeReal::from_secs(3.0))],
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_3d(size)
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id);
            });
        setup_eye(&test_context, view_id);
        test_context.handle_system_commands(&harness.ctx);
        harness.run_steps(10);
        snapshot_results.add(harness.try_snapshot("visible_time_range_3d/sliding_window_cursor_2"));
    }

    // Cursor to 4s → window covers T=[3s, 5s]
    {
        test_context.send_time_commands(
            test_context.active_store_id(),
            [TimeControlCommand::SetTime(TimeReal::from_secs(4.0))],
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_3d(size)
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id);
            });
        setup_eye(&test_context, view_id);
        test_context.handle_system_commands(&harness.ctx);
        harness.run_steps(10);
        snapshot_results.add(harness.try_snapshot("visible_time_range_3d/sliding_window_cursor_3"));
    }
}
