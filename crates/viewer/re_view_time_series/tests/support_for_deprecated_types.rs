//! Copy & pasted code from `basic.rs` to test that the deprecated "singular" types are still working.
#![allow(deprecated)]

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{
    test_context::{HarnessExt as _, TestContext},
    RecommendedView, ViewClass as _, ViewId,
};
use re_viewport_blueprint::{test_context_ext::TestContextExt as _, ViewBlueprint};

fn color_gradient0(step: i64) -> re_types::components::Color {
    re_types::components::Color::from_rgb((step * 8) as u8, 255 - (step * 8) as u8, 0)
}

fn scalar_for_properties_test(
    step: i64,
) -> (re_types::archetypes::Scalar, re_types::archetypes::Scalar) {
    (
        re_types::archetypes::Scalar::new((step as f64 / 5.0).sin()),
        re_types::archetypes::Scalar::new((step as f64 / 5.0).cos()),
    )
}

#[test]
fn test_line_properties_with_deprecated_types() {
    let mut test_context = get_test_context();

    let properties_static = re_types::archetypes::SeriesLine::new()
        .with_width(4.0)
        .with_color(re_types::components::Color::from_rgb(255, 0, 255))
        .with_name("static");
    test_context.log_entity("entity_static_props".into(), |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &properties_static)
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        let properties = re_types::archetypes::SeriesLine::new()
            .with_color(color_gradient0(step))
            .with_width((32.0 - step as f32) * 0.5)
            .with_name(format!("dynamic_{step}"));

        let (scalar_static, scalar_dymamic) = scalar_for_properties_test(step);
        test_context.log_entity("entity_static_props".into(), |builder| {
            builder.with_archetype(RowId::new(), timepoint.clone(), &scalar_static)
        });
        test_context.log_entity("entity_dynamic_props".into(), |builder| {
            builder
                .with_archetype(RowId::new(), timepoint.clone(), &properties)
                .with_archetype(RowId::new(), timepoint, &scalar_dymamic)
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "deprecated_line_properties",
        egui::vec2(300.0, 300.0),
        0.0,
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
fn test_point_properties_with_deprecated_types() {
    let mut test_context = get_test_context();

    let static_props = re_types::archetypes::SeriesPoint::new()
        .with_marker_size(4.0)
        .with_marker(re_types::components::MarkerShape::Cross)
        .with_color(re_types::components::Color::from_rgb(255, 0, 255))
        .with_name("static");

    test_context.log_entity("entity_static_props".into(), |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &static_props)
    });

    for step in 0..32 {
        let timepoint = TimePoint::from([(test_context.active_timeline(), step)]);

        let properties = re_types::archetypes::SeriesPoint::new()
            .with_color(color_gradient0(step))
            .with_marker_size((32.0 - step as f32) * 0.5)
            .with_marker(MARKER_LIST[step as usize % MARKER_LIST.len()])
            .with_name(format!("dynamic_{step}"));

        let (scalar_static, scalar_dymamic) = scalar_for_properties_test(step);
        test_context.log_entity("entity_static_props".into(), |builder| {
            builder.with_archetype(RowId::new(), timepoint.clone(), &scalar_static)
        });
        test_context.log_entity("entity_dynamic_props".into(), |builder| {
            builder
                .with_archetype(RowId::new(), timepoint.clone(), &properties)
                .with_archetype(RowId::new(), timepoint, &scalar_dymamic)
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "deprecated_point_properties",
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
    broken_pixels_percent: f64,
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
        broken_pixels_percent,
    );
}
