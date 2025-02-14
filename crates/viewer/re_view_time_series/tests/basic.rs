use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{test_context::TestContext, RecommendedView, ViewClass, ViewId};
use re_viewport_blueprint::{test_context_ext::TestContextExt, ViewBlueprint};

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

    test_context.log_entity("plots/line".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesLine::new(),
        )
    });
    test_context.log_entity("plots/point".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesPoint::new(),
        )
    });

    for i in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), i)]);

        match i {
            15 => {
                test_context.log_entity("plots".into(), |builder| {
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
                    re_types::archetypes::Scalar::default().with_many_scalar([
                        (i as f64 / 5.0).sin(),
                        (i as f64 / 5.0 + 1.0).cos(), // Shifted a bit to make the cap more visible
                    ])
                } else {
                    re_types::archetypes::Scalar::new((i as f64 / 5.0).sin())
                };

                test_context.log_entity("plots/line".into(), |builder| {
                    builder.with_archetype(RowId::new(), timepoint.clone(), &data)
                });
                test_context.log_entity("plots/point".into(), |builder| {
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
                "__two_series_per_entity"
            } else {
                ""
            }
        ),
        egui::vec2(300.0, 300.0),
    );
}

#[test]
fn test_line_properties() {
    let mut test_context = get_test_context();

    test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesLine::new()
                .with_width(4.0)
                .with_color(re_types::components::Color::from_rgb(255, 0, 255))
                .with_name("static"),
        )
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint.clone(),
                &re_types::archetypes::Scalar::new((step as f64 / 5.0).cos()),
            )
        });
        test_context.log_entity("not_what_is_displayed_dynamic_props".into(), |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    timepoint.clone(),
                    &re_types::archetypes::SeriesLine::new()
                        .with_color(color_gradient0(step))
                        .with_width((32.0 - step as f32) * 0.5)
                        // Only the first name will be shown, but should be handled gracefully.
                        .with_name(format!("dynamic_{step}")),
                )
                .with_archetype(
                    RowId::new(),
                    timepoint,
                    &re_types::archetypes::Scalar::new((step as f64 / 5.0).sin()),
                )
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "line_properties",
        egui::vec2(300.0, 300.0),
    );
}

#[test]
fn test_line_properties_two_series_per_entity() {
    let mut test_context = get_test_context();

    test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesLine::new()
                .with_many_width([4.0, 8.0])
                .with_many_color([
                    re_types::components::Color::from_rgb(255, 0, 255),
                    re_types::components::Color::from_rgb(0, 255, 0),
                ])
                .with_many_name(["static_0", "static_1"]),
        )
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint.clone(),
                &re_types::archetypes::Scalar::default()
                    .with_many_scalar([(step as f64 / 5.0).cos(), (step as f64 / 5.0).sin()]),
            )
        });
        test_context.log_entity("not_what_is_displayed_dynamic_props".into(), |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    timepoint.clone(),
                    &re_types::archetypes::SeriesLine::new()
                        .with_many_color([color_gradient0(step), color_gradient1(step)])
                        .with_many_width([(32.0 - step as f32) * 0.5, step as f32 * 0.5])
                        // Only the first set of name will be shown, but should be handled gracefully.
                        .with_many_name([format!("dynamic_{step}_0"), format!("dynamic_{step}_1")]),
                )
                .with_archetype(
                    RowId::new(),
                    timepoint,
                    &re_types::archetypes::Scalar::default().with_many_scalar([
                        (step as f64 / 5.0).sin() + 1.0,
                        (step as f64 / 5.0).cos() + 1.0,
                    ]),
                )
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "line_properties_two_series_per_entity",
        egui::vec2(300.0, 300.0),
    );
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
    let mut test_context = get_test_context();

    test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesPoint::new()
                .with_marker_size(4.0)
                .with_marker(re_types::components::MarkerShape::Cross)
                .with_color(re_types::components::Color::from_rgb(255, 0, 255))
                .with_name("static"),
        )
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint.clone(),
                &re_types::archetypes::Scalar::new((step as f64 / 5.0).cos()),
            )
        });
        test_context.log_entity("not_what_is_displayed_dynamic_props".into(), |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    timepoint.clone(),
                    &re_types::archetypes::SeriesPoint::new()
                        .with_color(color_gradient0(step))
                        .with_marker_size((32.0 - step as f32) * 0.5)
                        .with_marker(MARKER_LIST[step as usize % MARKER_LIST.len()])
                        // Only the first name will be shown, but should be handled gracefully.
                        .with_name(format!("dynamic_{step}")),
                )
                .with_archetype(
                    RowId::new(),
                    timepoint,
                    &re_types::archetypes::Scalar::new((step as f64 / 5.0).sin()),
                )
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "point_properties",
        egui::vec2(300.0, 300.0),
    );
}

#[test]
fn test_point_properties_two_series_per_entity() {
    let mut test_context = get_test_context();

    test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::SeriesPoint::new()
                .with_many_marker_size([4.0, 8.0])
                .with_many_marker([
                    re_types::components::MarkerShape::Cross,
                    re_types::components::MarkerShape::Plus,
                ])
                .with_many_color([
                    re_types::components::Color::from_rgb(255, 0, 255),
                    re_types::components::Color::from_rgb(0, 255, 0),
                ])
                .with_many_name(["static_0", "static_1"]),
        )
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        test_context.log_entity("not_what_is_displayed_static_props".into(), |builder| {
            builder.with_archetype(
                RowId::new(),
                timepoint.clone(),
                &re_types::archetypes::Scalar::default()
                    .with_many_scalar([(step as f64 / 5.0).cos(), (step as f64 / 5.0).sin()]),
            )
        });
        test_context.log_entity("not_what_is_displayed_dynamic_props".into(), |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    timepoint.clone(),
                    &re_types::archetypes::SeriesPoint::new()
                        .with_many_color([color_gradient0(step), color_gradient1(step)])
                        .with_many_marker_size([(32.0 - step as f32) * 0.5, step as f32 * 0.5])
                        .with_many_marker([
                            MARKER_LIST[step as usize % MARKER_LIST.len()],
                            MARKER_LIST[(step + 1) as usize % MARKER_LIST.len()],
                        ])
                        // Only the first name will be shown, but should be handled gracefully.
                        .with_many_name([format!("dynamic_{step}_0"), format!("dynamic_{step}_1")]),
                )
                .with_archetype(
                    RowId::new(),
                    timepoint,
                    &re_types::archetypes::Scalar::default().with_many_scalar([
                        (step as f64 / 5.0).sin() + 1.0,
                        (step as f64 / 5.0).cos() + 1.0,
                    ]),
                )
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "point_properties_two_series_per_entity",
        egui::vec2(300.0, 300.0),
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
        let view_blueprint =
            ViewBlueprint::new(TimeSeriesView::identifier(), RecommendedView::root());

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

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
                        .view_class_registry
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
    harness.snapshot(name);
}
