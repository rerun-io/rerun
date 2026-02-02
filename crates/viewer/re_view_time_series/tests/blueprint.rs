use re_log_types::{EntityPath, TimePoint, Timeline, TimelineName};
use re_sdk_types::archetypes::{self, Scalars, SeriesLines, SeriesPoints};
use re_sdk_types::blueprint;
use re_sdk_types::blueprint::archetypes::VisibleTimeRanges;
use re_sdk_types::datatypes::{self, TimeRange};
use re_sdk_types::{DynamicArchetype, VisualizableArchetype as _, components};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_blueprint_overrides_and_defaults_with_time_series() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let timeline = Timeline::log_tick();

    log_data(&mut test_context, timeline);

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline(*timeline.name())],
    );

    let view_id = setup_blueprint(&mut test_context, timeline.name(), None, None);
    let size = egui::vec2(300.0, 300.0);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "blueprint_overrides_and_defaults_with_time_series",
        size,
        None,
    ));

    for (range, name) in [
        (TimeRange::EVERYTHING, "everything"),
        (TimeRange::AT_CURSOR, "at_cursor"),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::CursorRelative(datatypes::TimeInt(-10)),
                end: datatypes::TimeRangeBoundary::CursorRelative(datatypes::TimeInt(10)),
            },
            "around_cursor",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(10)),
                end: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(20)),
            },
            "absolute",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(10)),
                end: datatypes::TimeRangeBoundary::Infinite,
            },
            "absolute_until_end",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Infinite,
                end: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(15)),
            },
            "start_until_absolute",
        ),
    ] {
        let view_id = setup_blueprint(&mut test_context, timeline.name(), Some(range), None);
        snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
            view_id,
            &format!("blueprint_overrides_and_defaults_with_time_series_{name}"),
            size,
            None,
        ));
    }
}

#[test]
pub fn test_custom_visible_time_range() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let timeline = Timeline::log_tick();

    log_data(&mut test_context, timeline);

    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetActiveTimeline(*timeline.name())],
    );

    let size = egui::vec2(300.0, 300.0);

    let data_ranges = [
        (TimeRange::EVERYTHING, "everything"),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::CursorRelative(datatypes::TimeInt(-10)),
                end: datatypes::TimeRangeBoundary::CursorRelative(datatypes::TimeInt(10)),
            },
            "around_cursor",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(10)),
                end: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(20)),
            },
            "absolute",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(10)),
                end: datatypes::TimeRangeBoundary::Infinite,
            },
            "absolute_until_end",
        ),
        (
            TimeRange {
                start: datatypes::TimeRangeBoundary::Infinite,
                end: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(15)),
            },
            "start_until_absolute",
        ),
    ];

    let mut snapshot_results = SnapshotResults::new();
    for (view_name, view_range) in [
        ("data", None),
        (
            "timeline",
            Some(TimeRange {
                start: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(0)),
                end: datatypes::TimeRangeBoundary::Absolute(datatypes::TimeInt(MAX_TIME)),
            }),
        ),
    ] {
        for (data_range, data_name) in &data_ranges {
            let view_id = setup_blueprint(
                &mut test_context,
                timeline.name(),
                view_range.clone(),
                Some(data_range.clone()),
            );
            snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
                view_id,
                &format!("visible_time_range_{data_name}_view_{view_name}"),
                size,
                None,
            ));
        }
    }
}

const MAX_TIME: i64 = 31;

fn log_data(test_context: &mut TestContext, timeline: re_log_types::Timeline) {
    for i in 0..=MAX_TIME {
        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;
        test_context.log_entity("plots/sin", |builder| {
            builder.with_archetype_auto_row(timepoint.clone(), &Scalars::single(t.sin()))
        });
        test_context.log_entity("plots/cos", |builder| {
            builder.with_archetype_auto_row(timepoint, &Scalars::single(t.cos()))
        });
    }
}

fn log_data_with_linear_speed(test_context: &mut TestContext, timeline: re_log_types::Timeline) {
    log_data(test_context, timeline);

    test_context.log_entity("plots/speed", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &SeriesLines::new()
                .with_colors([components::Color::from_rgb(255, 0, 0)])
                .with_names(["store_name"]),
        )
    });

    for i in 0..=MAX_TIME {
        let timepoint = TimePoint::from([(timeline, i)]);
        let t = i as f64 / 8.0;
        test_context.log_entity("plots/speed", |builder| {
            builder.with_archetype_auto_row(
                timepoint.clone(),
                &DynamicArchetype::new("custom")
                    .with_component::<components::LinearSpeed>("custom_component", [t.cos()]),
            )
        });
    }
}

fn setup_blueprint(
    test_context: &mut TestContext,
    timeline: &TimelineName,
    time_axis_view: Option<TimeRange>,
    visible_time_range: Option<TimeRange>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        // Overrides:
        ctx.save_visualizers(
            &EntityPath::from("plots/cos"),
            view.id,
            // Override color and markers for the `cos` plot and make it a points visualizer.
            [&archetypes::SeriesPoints::default()
                .with_colors([(0, 255, 0)])
                .with_markers([components::MarkerShape::Cross])],
        );

        // Override default color (should apply to the `sin` plot).
        ctx.save_blueprint_archetype(
            view.defaults_path.clone(),
            &archetypes::SeriesLines::default().with_colors([(0, 0, 255)]),
        );

        if let Some(time_axis_view) = time_axis_view {
            let time_axis = re_viewport_blueprint::ViewProperty::from_archetype::<
                blueprint::archetypes::TimeAxis,
            >(ctx.blueprint_db(), ctx.blueprint_query, view.id);

            time_axis.save_blueprint_component(
                ctx,
                &blueprint::archetypes::TimeAxis::descriptor_view_range(),
                &time_axis_view,
            );
        }

        if let Some(visible_time_range) = visible_time_range {
            let property = re_viewport_blueprint::ViewProperty::from_archetype::<VisibleTimeRanges>(
                ctx.blueprint_db(),
                ctx.blueprint_query,
                view.id,
            );

            property.save_blueprint_component(
                ctx,
                &VisibleTimeRanges::descriptor_ranges(),
                &blueprint::components::VisibleTimeRange(datatypes::VisibleTimeRange {
                    timeline: timeline.as_str().into(),
                    range: visible_time_range,
                }),
            );
        }

        blueprint.add_view_at_root(view)
    })
}

#[test]
pub fn test_explicit_component_mapping() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<TimeSeriesView>();

    let timeline = test_context.active_timeline().unwrap();
    log_data_with_linear_speed(&mut test_context, timeline);

    let view_id = setup_blueprint_with_explicit_mapping(&mut test_context);

    let size = egui::vec2(300.0, 300.0);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "explicit_component_mapping",
        size,
        None,
    ));
}

fn setup_blueprint_with_explicit_mapping(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        use re_sdk_types::blueprint::datatypes::{ComponentSourceKind, VisualizerComponentMapping};

        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        // Single line visualizer for the `sin` plot:
        // * scalar - map explicitly to Scalar component.
        // * color - explicitly use default (blue), ignore provided override.
        // * .. everything else to auto
        ctx.save_visualizers(
            &EntityPath::from("plots/sin"),
            view.id,
            [SeriesLines::new()
                .with_colors([components::Color::from_rgb(255, 0, 255)])
                .visualizer()
                .with_mappings([
                    VisualizerComponentMapping {
                        target: Scalars::descriptor_scalars().component.as_str().into(),
                        source_kind: ComponentSourceKind::SourceComponent,
                        source_component: None,
                        selector: None,
                    }
                    .into(),
                    VisualizerComponentMapping {
                        target: SeriesLines::descriptor_colors().component.as_str().into(),
                        source_kind: ComponentSourceKind::Default,
                        source_component: None,
                        selector: None,
                    }
                    .into(),
                ])],
        );

        // Two visualizers for the `speed` plot:
        // * Points:
        //    * scalar - map to `LinearSpeed` component.
        //    * color - explicitly use Default, leading to the fallback since the view default is on lines.
        //    * … everything else is auto, which will not pick up anything from the store.
        // * Lines:
        //    * scalar - map to `LinearSpeed` component.
        //    * color - explicitly use Override
        //    * … everything else is auto, which will pick up the SeriesLines name from the store.
        let scalar_mapping = VisualizerComponentMapping {
            target: Scalars::descriptor_scalars().component.as_str().into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some("custom:custom_component".into()),
            selector: None,
        };
        ctx.save_visualizers(
            &EntityPath::from("plots/speed"),
            view.id,
            [
                SeriesPoints::new().visualizer().with_mappings([
                    blueprint::components::VisualizerComponentMapping(scalar_mapping.clone()),
                    blueprint::components::VisualizerComponentMapping(VisualizerComponentMapping {
                        target: SeriesPoints::descriptor_colors().component.as_str().into(),
                        source_kind: ComponentSourceKind::Default,
                        source_component: None,
                        selector: None,
                    }),
                ]),
                SeriesLines::new()
                    .with_colors([components::Color::from_rgb(0, 255, 0)])
                    .visualizer()
                    .with_mappings([
                        blueprint::components::VisualizerComponentMapping(scalar_mapping),
                        blueprint::components::VisualizerComponentMapping(
                            VisualizerComponentMapping {
                                target: SeriesLines::descriptor_colors().component.as_str().into(),
                                source_kind: ComponentSourceKind::Override,
                                source_component: None,
                                selector: None,
                            },
                        ),
                    ]),
            ],
        );

        // No visualization at all for plots/cos.
        ctx.save_visualizers(
            &EntityPath::from("plots/cos"),
            view.id,
            std::iter::empty::<re_sdk_types::Visualizer>(),
        );

        // Set a default color on the view (blue)
        ctx.save_blueprint_archetype(
            view.defaults_path.clone(),
            &archetypes::SeriesLines::default().with_colors([(0, 0, 255)]),
        );

        blueprint.add_view_at_root(view)
    })
}
