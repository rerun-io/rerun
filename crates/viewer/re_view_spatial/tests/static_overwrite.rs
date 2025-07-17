//! When logging static data twice, the second write wins and overwrites the first one.
//! This test ensures that overrides and defaults still work in this setting, after
//! we had a bug in this logic in the past: <https://github.com/rerun-io/rerun/pull/7199>

use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_types::archetypes;
use re_view_spatial::SpatialView3D;
use re_viewer_context::{
    ViewClass as _, ViewId, external::egui_kittest::SnapshotOptions, test_context::TestContext,
};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

const SNAPSHOT_SIZE: egui::Vec2 = egui::vec2(300.0, 300.0);

fn log_twice(test_context: &mut TestContext, entity_path: &EntityPath) {
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::Points3D::new([(0.0, 1.0, 0.0), (1.0, 1.0, 1.0)]),
        )
    });

    // Log it again, to ensure that the newest one is visible.
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::Points3D::new([(0.0, 1.0, 0.0), (1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
        )
    });
}

fn setup_blueprint(
    test_context: &mut TestContext,
    entity_path: &EntityPath,
    radius_default: Option<&archetypes::Points3D>,
    color_override: Option<&archetypes::Points3D>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView3D::identifier());

        if let Some(radius_default) = radius_default {
            ctx.save_blueprint_archetype(view.defaults_path.clone(), radius_default);
        }

        if let Some(color_override) = color_override {
            let override_path = ViewContents::override_path_for_entity(view.id, entity_path);
            ctx.save_blueprint_archetype(override_path.clone(), color_override);
        }

        blueprint.add_view_at_root(view)
    })
}

#[test]
pub fn test_static_overwrite_original() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    let entity_path = EntityPath::from("points");

    log_twice(&mut test_context, &entity_path);

    let view_id = setup_blueprint(&mut test_context, &entity_path, None, None);

    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "static_overwrite_original",
        SNAPSHOT_SIZE,
    );
}

#[test]
pub fn test_static_overwrite_radius_default() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    let entity_path = EntityPath::from("points");

    log_twice(&mut test_context, &entity_path);

    let radius_default = archetypes::Points3D::default().with_radii([0.25]);
    let view_id = setup_blueprint(&mut test_context, &entity_path, Some(&radius_default), None);

    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "static_overwrite_radius_default",
        SNAPSHOT_SIZE,
    );
}

#[test]
pub fn test_static_overwrite_color_override() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    let entity_path = EntityPath::from("points");

    log_twice(&mut test_context, &entity_path);

    let color_override = archetypes::Points3D::default()
        .with_colors([[0, 255, 0]])
        .with_radii([0.25]);
    let view_id = setup_blueprint(&mut test_context, &entity_path, None, Some(&color_override));

    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "static_overwrite_color_override",
        SNAPSHOT_SIZE,
    );
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

    let raw_input = harness.input_mut();
    raw_input
        .events
        .push(egui::Event::PointerMoved((100.0, 100.0).into()));
    raw_input.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::Vec2::UP * 3.1,
        modifiers: egui::Modifiers::default(),
    });
    harness.run_steps(8);

    let broken_pixels_fraction = 0.004;

    let options = SnapshotOptions::new()
        .failed_pixel_count_threshold((size.x * size.y * broken_pixels_fraction).round() as usize);

    harness.snapshot_options(name, &options);
}
