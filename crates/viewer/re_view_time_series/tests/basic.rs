use re_chunk_store::{RowId, external::re_chunk::ChunkBuilder};
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_test_context::{TestContext, external::egui_kittest::SnapshotOptions};
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

fn color_gradient0(step: i64) -> re_types::components::Color {
    re_types::components::Color::from_rgb((step * 8) as u8, 255 - (step * 8) as u8, 0)
}

fn color_gradient1(step: i64) -> re_types::components::Color {
    re_types::components::Color::from_rgb(255 - (step * 8) as u8, 0, (step * 8) as u8)
}

#[test]
pub fn test_clear_series_points_and_line() {
    for two_series_per_entity in [false, true] {
        test_clear_series_points_and_line_impl(two_series_per_entity);
    }
}

fn test_clear_series_points_and_line_impl(two_series_per_entity: bool) {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    // TODO(#10512): Potentially fix up this after we have "markers".
    // There are some intricacies involved with this test. `SeriesLines` and
    // `SeriesPoints` can both be logged without any associated data (all
    // fields are optional). Now that indicators are gone, no data is logged
    // at all when no fields are specified.
    //
    // The reason why `SeriesLines` still shows up is because it is the fallback
    // visualizer for scalar values. We force `SeriesPoints` to have data, by
    // explicitly setting the marker shape to circle.
    test_context.log_entity("plots/line", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesLines::new(),
        )
    });
    test_context.log_entity("plots/point", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesPoints::new()
                .with_markers([re_types::components::MarkerShape::Circle]),
        )
    });

    for i in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), i)]);

        match i {
            15 => {
                test_context.log_entity("plots", |builder| {
                    builder.with_archetype(
                        RowId::new(),
                        timepoint,
                        &re_types::archetypes::Clear::new(true),
                    )
                });
            }
            16..18 => {
                // Gap.
            }
            _ => {
                let data = if two_series_per_entity {
                    re_types::archetypes::Scalars::default().with_scalars([
                        (i as f64 / 5.0).sin(),
                        (i as f64 / 5.0 + 1.0).cos(), // Shifted a bit to make the cap more visible
                    ])
                } else {
                    re_types::archetypes::Scalars::single((i as f64 / 5.0).sin())
                };

                test_context.log_entity("plots/line", |builder| {
                    builder.with_archetype(RowId::new(), timepoint.clone(), &data)
                });
                test_context.log_entity("plots/point", |builder| {
                    builder.with_archetype(RowId::new(), timepoint, &data)
                });
            }
        }
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        &format!(
            "clear_series_points_and_line{}",
            if two_series_per_entity {
                "_two_series_per_entity"
            } else {
                ""
            }
        ),
        egui::vec2(300.0, 300.0),
        if two_series_per_entity { 5 } else { 2 },
    );
}

fn scalars_for_properties_test(
    step: i64,
    multiple_scalars: bool,
) -> (re_types::archetypes::Scalars, re_types::archetypes::Scalars) {
    if multiple_scalars {
        (
            re_types::archetypes::Scalars::new([
                (step as f64 / 5.0).sin() + 1.0,
                (step as f64 / 5.0).cos() + 1.0,
            ]),
            re_types::archetypes::Scalars::new([
                (step as f64 / 5.0).cos(),
                (step as f64 / 5.0).sin(),
            ]),
        )
    } else {
        (
            re_types::archetypes::Scalars::single((step as f64 / 5.0).sin()),
            re_types::archetypes::Scalars::single((step as f64 / 5.0).cos()),
        )
    }
}

#[test]
fn test_line_properties() {
    for multiple_properties in [false, true] {
        let multiple_scalars = true;
        test_line_properties_impl(multiple_properties, multiple_scalars);
    }
}

fn test_line_properties_impl(multiple_properties: bool, multiple_scalars: bool) {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let properties_static = if multiple_properties {
        re_types::archetypes::SeriesLines::new()
            .with_widths([4.0, 8.0])
            .with_colors([
                re_types::components::Color::from_rgb(255, 0, 255),
                re_types::components::Color::from_rgb(0, 255, 0),
            ])
            .with_names(["static_0", "static_1"])
    } else {
        re_types::archetypes::SeriesLines::new()
            .with_widths([4.0])
            .with_colors([re_types::components::Color::from_rgb(255, 0, 255)])
            .with_names(["static"])
    };
    test_context.log_entity("entity_static_props", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &properties_static)
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        let properties = if multiple_properties {
            re_types::archetypes::SeriesLines::new()
                .with_colors([color_gradient0(step), color_gradient1(step)])
                .with_widths([(32.0 - step as f32) * 0.5, step as f32 * 0.5])
                // Only the first set of name will be shown, but should be handled gracefully.
                .with_names([format!("dynamic_{step}_0"), format!("dynamic_{step}_1")])
        } else {
            re_types::archetypes::SeriesLines::new()
                .with_colors([color_gradient0(step)])
                .with_widths([(32.0 - step as f32) * 0.5])
                .with_names([format!("dynamic_{step}")])
        };

        let (scalars_static, scalars_dynamic) = scalars_for_properties_test(step, multiple_scalars);
        test_context.log_entity("entity_static_props", |builder| {
            builder.with_archetype(RowId::new(), timepoint.clone(), &scalars_static)
        });
        test_context.log_entity("entity_dynamic_props", |builder| {
            builder
                .with_archetype(RowId::new(), timepoint.clone(), &properties)
                .with_archetype(RowId::new(), timepoint, &scalars_dynamic)
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    let mut name = "line_properties".to_owned();
    if multiple_properties {
        name += "_multiple_properties";
    }
    if multiple_scalars {
        name += "_two_series_per_entity";
    }
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        &name,
        egui::vec2(300.0, 300.0),
        if multiple_scalars { 5 } else { 0 },
    );
}

/// Test the per series visibility setting
#[test]
fn test_per_series_visibility() {
    for (name, visibility) in [
        ("per_series_visibility_show_second_only", vec![false, true]),
        ("per_series_visibility_splat_false", vec![false]),
        ("per_series_visibility_splat_true", vec![true]),
    ] {
        let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

        test_context.log_entity("plots", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::SeriesLines::new().with_visible_series(visibility),
            )
        });

        for step in 0..32 {
            let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);
            let (scalars, _) = scalars_for_properties_test(step, true);
            test_context.log_entity("plots", |builder| {
                builder.with_archetype(RowId::new(), timepoint.clone(), &scalars)
            });
        }

        let view_id = setup_blueprint(&mut test_context);
        run_view_ui_and_save_snapshot(
            &mut test_context,
            view_id,
            name,
            egui::vec2(300.0, 300.0),
            0,
        );
    }
}

const MARKER_LIST: [re_types::components::MarkerShape; 10] = [
    re_types::components::MarkerShape::Circle,
    re_types::components::MarkerShape::Diamond,
    re_types::components::MarkerShape::Square,
    re_types::components::MarkerShape::Cross,
    re_types::components::MarkerShape::Plus,
    re_types::components::MarkerShape::Up,
    re_types::components::MarkerShape::Down,
    re_types::components::MarkerShape::Left,
    re_types::components::MarkerShape::Right,
    re_types::components::MarkerShape::Asterisk,
];

#[test]
fn test_point_properties() {
    for multiple_properties in [false, true] {
        let multiple_scalars = true;
        test_point_properties_impl(multiple_properties, multiple_scalars);
    }
}

fn test_point_properties_impl(multiple_properties: bool, multiple_scalars: bool) {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    let static_props = if multiple_properties {
        re_types::archetypes::SeriesPoints::new()
            .with_marker_sizes([4.0, 8.0])
            .with_markers([
                re_types::components::MarkerShape::Cross,
                re_types::components::MarkerShape::Plus,
            ])
            .with_colors([
                re_types::components::Color::from_rgb(255, 0, 255),
                re_types::components::Color::from_rgb(0, 255, 0),
            ])
            .with_names(["static_0", "static_1"])
    } else {
        re_types::archetypes::SeriesPoints::new()
            .with_marker_sizes([4.0])
            .with_markers([re_types::components::MarkerShape::Cross])
            .with_colors([re_types::components::Color::from_rgb(255, 0, 255)])
            .with_names(["static"])
    };

    test_context.log_entity("entity_static_props", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &static_props)
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        let properties = if multiple_properties {
            re_types::archetypes::SeriesPoints::new()
                .with_colors([color_gradient0(step), color_gradient1(step)])
                .with_marker_sizes([(32.0 - step as f32) * 0.5, step as f32 * 0.5])
                .with_markers([
                    MARKER_LIST[step as usize % MARKER_LIST.len()],
                    MARKER_LIST[(step + 1) as usize % MARKER_LIST.len()],
                ])
                .with_names([format!("dynamic_{step}_0"), format!("dynamic_{step}_1")])
        } else {
            re_types::archetypes::SeriesPoints::new()
                .with_colors([color_gradient0(step)])
                .with_marker_sizes([(32.0 - step as f32) * 0.5])
                .with_markers([MARKER_LIST[step as usize % MARKER_LIST.len()]])
                .with_names([format!("dynamic_{step}")])
        };

        let (scalars_static, scalars_dynamic) = scalars_for_properties_test(step, multiple_scalars);
        test_context.log_entity("entity_static_props", |builder| {
            builder.with_archetype(RowId::new(), timepoint.clone(), &scalars_static)
        });
        test_context.log_entity("entity_dynamic_props", |builder| {
            builder
                .with_archetype(RowId::new(), timepoint.clone(), &properties)
                .with_archetype(RowId::new(), timepoint, &scalars_dynamic)
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    let mut name = "point_properties".to_owned();
    if multiple_properties {
        name += "_multiple_properties";
    }
    if multiple_scalars {
        name += "_two_series_per_entity";
    }
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        &name,
        egui::vec2(300.0, 300.0),
        5, // Allow 5 broken pixels
    );
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TimeSeriesView::identifier(),
        ))
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
    num_allowed_broken_pixels: usize,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.snapshot_options(
        name,
        &SnapshotOptions::new().failed_pixel_count_threshold(num_allowed_broken_pixels),
    );
}

#[test]
fn test_bootstrapped_secondaries() {
    for partial_range in [false, true] {
        test_bootstrapped_secondaries_impl(partial_range);
    }
}

fn test_bootstrapped_secondaries_impl(partial_range: bool) {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();

    fn with_scalar(builder: ChunkBuilder, value: i64) -> ChunkBuilder {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(Timeline::log_tick(), value)]),
            &re_types::archetypes::Scalars::new([value as f64]),
        )
    }

    test_context.log_entity("scalars", |builder| {
        let mut builder = builder
            .with_archetype(
                RowId::new(),
                TimePoint::from([(Timeline::log_tick(), 0)]),
                &re_types::archetypes::SeriesLines::new()
                    .with_widths([5.0])
                    .with_colors([re_types::components::Color::from_rgb(0, 255, 255)])
                    .with_names(["muh_scalars_from_0"]),
            )
            .with_archetype(
                RowId::new(),
                TimePoint::from([(Timeline::log_tick(), 45)]),
                &re_types::archetypes::SeriesLines::new()
                    .with_widths([5.0])
                    .with_colors([re_types::components::Color::from_rgb(255, 0, 255)])
                    .with_names(["muh_scalars_from_45"]),
            );
        for i in 0..10 {
            builder = with_scalar(builder, i * 10);
        }
        builder
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());

        if partial_range {
            let override_path =
                ViewContents::override_path_for_entity(view.id, &EntityPath::from("scalars"));
            ctx.save_blueprint_archetype(
                override_path.clone(),
                &re_types::blueprint::archetypes::VisibleTimeRanges::new([
                    re_types::blueprint::components::VisibleTimeRange(
                        re_types::datatypes::VisibleTimeRange {
                            timeline: "log_tick".into(),
                            range: re_types::datatypes::TimeRange {
                                start: re_types::datatypes::TimeRangeBoundary::Absolute(70.into()),
                                end: re_types::datatypes::TimeRangeBoundary::Infinite,
                            },
                        },
                    ),
                ]),
            );
        }

        blueprint.add_view_at_root(view)
    });

    let name = if partial_range {
        "bootstrapped_secondaries_partial"
    } else {
        "bootstrapped_secondaries_full"
    };
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        name,
        egui::vec2(300.0, 300.0),
        2,
    );
}
