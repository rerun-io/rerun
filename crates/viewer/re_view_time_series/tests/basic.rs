use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{
    ViewClass as _, ViewId,
    test_context::{HarnessExt as _, TestContext},
};
use re_viewport_blueprint::{ViewBlueprint, test_context_ext::TestContextExt as _};

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
    let mut test_context = get_test_context();

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
            &re_types::archetypes::SeriesPoints::new(),
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
        if two_series_per_entity {
            0.00006
        } else {
            0.00002
        },
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
    let mut test_context = get_test_context();

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
        if multiple_scalars { 0.00006 } else { 0.0 },
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
        let mut test_context = get_test_context();

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
            0.0,
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
    let mut test_context = get_test_context();

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
        0.00006, // Allow 5 broken pixels
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<TimeSeriesView>();

    test_context
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
    broken_pixels_fraction: f64,
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
                        .get_class_or_log_error(TimeSeriesView::identifier());

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
    harness.snapshot_with_broken_pixels_threshold(
        name,
        (size.x * size.y) as u64,
        broken_pixels_fraction,
    );
}
