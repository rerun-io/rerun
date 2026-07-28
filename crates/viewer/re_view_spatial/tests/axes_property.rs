//! Tests for handling of the view coordinates (the [`SpatialInformation`] axes) in the 3D view:
//! * left-handed coordinate systems emit a warning, degenerate ones an error.
//! * different (valid) orientations actually change what's rendered.

use re_log_types::{EntityPath, EntityPathFilter, TimePoint};
use re_sdk_types::blueprint::archetypes::SpatialInformation;
use re_sdk_types::components::ViewCoordinates;
use re_sdk_types::datatypes::ViewDir;
use re_sdk_types::{archetypes, components};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId, ViewerReportSeverity};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

fn setup_scene(test_context: &mut TestContext) {
    // Axis-colored arrows from the origin: red=+X, green=+Y, blue=+Z. These make the
    // orientation of the scene immediately obvious when the view coordinates change.
    let arrows =
        archetypes::Arrows3D::from_vectors([(1.5, 0.0, 0.0), (0.0, 1.5, 0.0), (0.0, 0.0, 1.5)])
            .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF]);

    // Plus an asymmetric, axis-colored set of boxes near the tip of each axis.
    let boxes = archetypes::Boxes3D::from_centers_and_half_sizes(
        [(1.5, 0.0, 0.0), (0.0, 1.5, 0.0), (0.0, 0.0, 1.5)],
        [(0.3, 0.1, 0.1), (0.1, 0.3, 0.1), (0.1, 0.1, 0.3)],
    )
    .with_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF])
    .with_fill_mode(components::FillMode::Solid);

    test_context.log_entity("axes", |builder| {
        builder.with_archetype_auto_row(TimePoint::STATIC, &arrows)
    });
    test_context.log_entity("boxes", |builder| {
        builder.with_archetype_auto_row(TimePoint::STATIC, &boxes)
    });
}

fn add_3d_view(test_context: &mut TestContext, axes: Option<ViewCoordinates>) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: EntityPath::root(),
                query_filter: EntityPathFilter::all(),
            },
        );
        let view_id = view.id;

        if let Some(axes) = axes {
            ViewProperty::from_archetype_for_view::<SpatialInformation>(ctx, view_id)
                .save_blueprint_component(ctx, &SpatialInformation::descriptor_axes(), &axes);
        }

        blueprint.add_view_at_root(view)
    })
}

/// Runs the view and returns the view-level reports (severity + summary) it emitted.
fn view_reports_for_axes(axes: Option<ViewCoordinates>) -> Vec<(ViewerReportSeverity, String)> {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();
    setup_scene(&mut test_context);
    let view_id = add_3d_view(&mut test_context, axes);

    let mut harness = test_context
        .setup_kittest_for_rendering_3d([300.0, 200.0])
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                test_context.ui_for_single_view(ui, ctx, view_id);
            });
        });
    harness.run();

    test_context
        .view_states
        .lock()
        .view_reports(&test_context.recording_store_id, view_id)
        .iter()
        .map(|report| (report.severity, report.summary.clone()))
        .collect()
}

#[test]
fn test_right_handed_view_coordinates_emit_no_report() {
    // X=Right, Y=Forward, Z=Up — a right-handed system, the default.
    let reports = view_reports_for_axes(Some(ViewCoordinates::new(
        ViewDir::Right,
        ViewDir::Forward,
        ViewDir::Up,
    )));
    assert!(reports.is_empty(), "expected no reports, got: {reports:?}");
}

#[test]
fn test_left_handed_view_coordinates_emit_warning() {
    // X=Right, Y=Up, Z=Forward — a left-handed system.
    let reports = view_reports_for_axes(Some(ViewCoordinates::new(
        ViewDir::Right,
        ViewDir::Up,
        ViewDir::Forward,
    )));
    assert_eq!(reports.len(), 1, "expected one report, got: {reports:?}");
    assert_eq!(reports[0].0, ViewerReportSeverity::Warning);
    assert_eq!(reports[0].1, "Unsupported left-handed coordinates");
}

#[test]
fn test_invalid_view_coordinates_emit_error() {
    // Two axes along the same dimension (X=Up, Y=Up) — a degenerate/invalid system.
    let reports = view_reports_for_axes(Some(ViewCoordinates::new(
        ViewDir::Up,
        ViewDir::Up,
        ViewDir::Forward,
    )));
    assert_eq!(reports.len(), 1, "expected one report, got: {reports:?}");
    assert_eq!(reports[0].0, ViewerReportSeverity::Error);
    assert_eq!(reports[0].1, "Invalid view coordinates");
}

fn snapshot_orientation(name: &str, axes: ViewCoordinates) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();
    setup_scene(&mut test_context);
    let view_id = add_3d_view(&mut test_context, Some(axes));

    let mut harness = test_context
        .setup_kittest_for_rendering_3d([400.0, 300.0])
        .build_ui(|ui| {
            test_context.run_ui(ui, |ctx, ui| {
                test_context.ui_for_single_view(ui, ctx, view_id);
            });
        });
    harness.run();
    harness.snapshot(name);
}

// The following are all right-handed, but with different up-axes — the rendered scene should differ between them.
// Kept as separate tests, so the 3D view resets completely again, skpping any transitional animation.

#[test]
fn test_orientation_rfu_z_up() {
    snapshot_orientation(
        "orientation_rfu_z_up",
        ViewCoordinates::new(ViewDir::Right, ViewDir::Forward, ViewDir::Up),
    );
}

#[test]
fn test_orientation_rub_y_up() {
    snapshot_orientation(
        "orientation_rub_y_up",
        ViewCoordinates::new(ViewDir::Right, ViewDir::Up, ViewDir::Back),
    );
}

#[test]
fn test_orientation_flu_x_up() {
    snapshot_orientation(
        "orientation_flu_x_up",
        ViewCoordinates::new(ViewDir::Forward, ViewDir::Left, ViewDir::Up),
    );
}
