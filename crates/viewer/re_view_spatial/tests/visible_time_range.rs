use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimeInt, TimePoint, TimeReal, Timeline};
use re_types::archetypes::Points2D;
use re_types::datatypes::VisibleTimeRange;
use re_types::{components, Archetype};
use re_view_spatial::SpatialView2D;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass, ViewId};
use re_viewport_blueprint::test_context_ext::TestContextExt;
use re_viewport_blueprint::ViewBlueprint;

/// # Range: partial primary and secondary updates
///
/// Checks that inter- and intra-timestamp partial updates are properly handled by range queries,
/// end-to-end: all the way to the views and the renderer.
fn intra_timestamp_data(test_context: &mut TestContext) {
    let timeline = Timeline::new_sequence("frame");
    let points_path = EntityPath::from("points");

    let frame = |sequence: i64| {
        TimePoint::default().with(
            timeline,
            TimeInt::from_sequence(sequence.try_into().unwrap()),
        )
    };

    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(42),
            &Points2D::new([(0.0, 0.0), (1.0, 1.0)]).with_colors([[255, 0, 0]]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_sparse_component_batches(
            RowId::new(),
            frame(42),
            [(
                Points2D::descriptor_radii(),
                Some(&components::Radius::from(0.1) as _),
            )],
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_sparse_component_batches(
            RowId::new(),
            frame(44),
            [(
                Points2D::descriptor_colors(),
                Some(&components::Color::from([0, 0, 255]) as _),
            )],
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(45),
            &Points2D::new([(0.0, 1.0), (1.0, 0.0)]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_sparse_component_batches(
            RowId::new(),
            frame(45),
            [(
                Points2D::descriptor_radii(),
                Some(&components::Radius::from(0.2) as _),
            )],
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_sparse_component_batches(
            RowId::new(),
            frame(46),
            [(
                Points2D::descriptor_radii(),
                Some(&components::Radius::from(0.2) as _),
            )],
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(46),
            &Points2D::new([(0.0, 2.0), (1.0, 2.0)]),
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_sparse_component_batches(
            RowId::new(),
            frame(46),
            [(
                Points2D::descriptor_radii(),
                Some(&components::Radius::from(0.15) as _),
            )],
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_sparse_component_batches(
            RowId::new(),
            frame(46),
            [(
                Points2D::descriptor_colors(),
                Some(&components::Color::from([0, 255, 0]) as _),
            )],
        )
    });
    test_context.log_entity(points_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            frame(46),
            &Points2D::new([(2.0, 2.0), (2.0, 0.0)]),
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
        "intra_timestamp/42:42",
        intra_timestamp_data,
        Some(range_absolute(42, 42)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/43:44",
        intra_timestamp_data,
        Some(range_absolute(43, 44)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/42:44",
        intra_timestamp_data,
        Some(range_absolute(42, 44)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/43:45",
        intra_timestamp_data,
        Some(range_absolute(43, 45)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/46:46",
        intra_timestamp_data,
        Some(range_absolute(46, 46)),
        None,
    );

    run_visible_time_range_test(
        "intra_timestamp/infinity:infinity",
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
    let timeline = Timeline::new_temporal("timestamp");
    {
        test_context.log_entity("bg".into(), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Boxes2D::from_mins_and_sizes([(0.0, 0.0)], [(100.0, 100.0)])
                    .with_colors([0x333333FF])
                    .with_radii([2.5]),
            )
        });
        for i in 0..10 {
            let x = i as f32 * 10.0 + 5.0;
            let y_red = 40.0;
            let y_green = 60.0;
            let time_point =
                TimePoint::default().with(timeline, TimeInt::from_seconds(i.try_into().unwrap()));

            for y in [y_green, y_red] {
                test_context.log_entity(
                    format!("point_{:02}_{:02}", i, y as i32).into(),
                    |builder| {
                        builder.with_archetype(
                            RowId::new(),
                            TimePoint::default(),
                            &re_types::archetypes::Points2D::new([(x, y)])
                                .with_colors([0x555555FF])
                                .with_radii([2.5])
                                .with_draw_order(1.0),
                        )
                    },
                );
            }

            {
                let time_point = time_point.clone();
                test_context.log_entity("red".into(), |builder| {
                    builder.with_archetype(
                        RowId::new(),
                        time_point,
                        &re_types::archetypes::Points2D::new([(x, y_red)])
                            .with_colors([0xFF0000FF])
                            .with_radii([2.5])
                            .with_draw_order(3.0),
                    )
                });
            }

            test_context.log_entity("green".into(), |builder| {
                builder.with_archetype(
                    RowId::new(),
                    time_point,
                    &re_types::archetypes::Points2D::new([(x, y_green)])
                        .with_colors([0x00FF00FF])
                        .with_radii([2.5])
                        .with_draw_order(3.0),
                )
            })
        }
    }

    let mut time_ctrl = test_context.recording_config.time_ctrl.write();
    time_ctrl.set_timeline(timeline);
    time_ctrl.set_time(TimeReal::from_seconds(4.5));
}

#[test]
fn test_visible_time_range_latest_at() {
    let range_absolute = |timeline: &str, start: i64, end: i64| VisibleTimeRange {
        timeline: timeline.into(),
        range: re_types::datatypes::TimeRange {
            start: re_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_milliseconds(start.try_into().unwrap()).into(),
            ),
            end: re_types::datatypes::TimeRangeBoundary::Absolute(
                TimeInt::from_milliseconds(end.try_into().unwrap()).into(),
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
                    TimeInt::from_milliseconds((-1500).try_into().unwrap()).into(),
                ),
                end: re_types::datatypes::TimeRangeBoundary::CursorRelative(
                    TimeInt::from_milliseconds(0.try_into().unwrap()).into(),
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
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        name,
        egui::vec2(300.0, 150.0) * 2.0,
    );
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
        let view_blueprint =
            ViewBlueprint::new(SpatialView2D::identifier(), RecommendedView::root());

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        if let Some(time_range) = time_range {
            let visible_time_range_list = vec![re_types::blueprint::components::VisibleTimeRange(
                time_range,
            )];
            let property_path = re_viewport_blueprint::entity_path_for_view_property(
                view_id,
                ctx.store_context.blueprint.tree(),
                re_types::blueprint::archetypes::VisibleTimeRanges::name(),
            );

            ctx.save_blueprint_component(&property_path, &visible_time_range_list);
        }

        if let Some(green_time_range) = green_time_range {
            let visible_time_range_list = vec![re_types::blueprint::components::VisibleTimeRange(
                green_time_range,
            )];
            let property_path = view_id
                .as_entity_path()
                .join(&EntityPath::from("ViewContents/recursive_overrides/green"));

            ctx.save_blueprint_component(&property_path, &visible_time_range_list);
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
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    let view_class = ctx
                        .view_class_registry()
                        .get_class_or_log_error(SpatialView2D::identifier());

                    let view_blueprint = ViewBlueprint::try_from_db(
                        view_id,
                        ctx.store_context.blueprint,
                        ctx.blueprint_query,
                    )
                    .expect("we just created that view");

                    let mut view_states = test_context.view_states.lock();
                    let view_state = view_states.get_mut_or_create(view_id, view_class);

                    let (view_query, system_execution_output) =
                        re_viewport::execute_systems_for_view(ctx, &view_blueprint, view_state);

                    view_class
                        .ui(ctx, ui, view_state, &view_query, system_execution_output)
                        .expect("failed to run graph view ui");
                });

                test_context.handle_system_commands();
            });
        });

    harness.run();
    harness.snapshot(name);
}
