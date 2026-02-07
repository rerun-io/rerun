//! Tests for the origin heuristics feature in the viewer.
//!
//! The origin heuristics feature is used to automatically set the view origin
//! for views based on the data in the log.

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_image() -> re_sdk_types::archetypes::Image {
    let image = ndarray::Array3::from_shape_fn((256, 256, 3), |(y, x, c)| match c {
        0 => x as u8,
        1 => y as u8,
        2 => 128,
        _ => unreachable!(),
    });

    re_sdk_types::archetypes::Image::from_color_model_and_tensor(
        re_sdk_types::datatypes::ColorModel::RGB,
        image,
    )
    .expect("Failed to create image")
}

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_selection_panel_opened(true);

    // Log some data
    harness.log_entity("/", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(0.0, 0.0, 0.0)],
                [(0.3, 0.3, 0.3)],
            )
            .with_colors([0xFF9001FF]),
        )
    });

    harness.log_entity("/world", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::ViewCoordinates::RIGHT_HAND_Y_DOWN(),
        )
    });

    harness.log_entity("/world/camera", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Transform3D::from_rotation(
                re_sdk_types::components::RotationAxisAngle::new(
                    [-1.0, 0.9, 0.0],
                    std::f32::consts::PI / 2.0,
                ),
            ),
        )
    });

    harness.log_entity("/world/camera/image", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Pinhole::from_focal_length_and_resolution(
                [128.0, 128.0],
                [256.0, 256.0],
            )
            .with_principal_point([128.0, 128.0])
            .with_image_plane_distance(0.2),
        )
    });

    harness.log_entity("/world/camera/image", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &make_test_image())
    });

    harness.log_entity("/world/camera/image/keypoint", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Points2D::new([(10.0, 10.0), (128.0, -0.0), (50.0, -50.0)])
                .with_radii([-10.0, -10.0, -10.0])
                .with_colors([0xFF9001FF, 0x9001FFFF, 0x90FF01FF]),
        )
    });

    // Set up a multi-view blueprint
    harness.clear_current_blueprint();

    let mut view3d =
        ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
    view3d.display_name = Some("3D view".into());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_view_at_root(view3d);
    });

    harness
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_keypoint_3d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    let keypoint = harness
        .blueprint_tree()
        .get_label("keypoint")
        .rect()
        .left_center();
    harness.right_click_at(keypoint);
    harness.hover_label_contains("Add to new view");
    harness.click_label("3D");
    harness.snapshot_app("origin_keypoint_3d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_keypoint_2d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    let keypoint = harness
        .blueprint_tree()
        .get_label("keypoint")
        .rect()
        .left_center();
    harness.right_click_at(keypoint);
    harness.hover_label_contains("Add to new view");
    harness.click_label("2D");
    harness.remove_cursor();
    harness.snapshot_app("origin_keypoint_2d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_image_3d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    // Close the selection panel before clicking on the image entity
    // because rendering its download button requires the UI to be on
    // the main thread. We are in a tokio test and it will crash.
    harness.set_selection_panel_opened(false);

    harness.blueprint_tree().right_click_label("image");
    harness.hover_label_contains("Add to new view");
    harness.click_label("3D");
    harness.set_selection_panel_opened(true);
    harness.snapshot_app("origin_image_3d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_image_2d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    // Close the selection panel before clicking on the image entity
    // because rendering its download button requires the UI to be on
    // the main thread. We are in a tokio test and it will crash.
    harness.set_selection_panel_opened(false);

    harness.blueprint_tree().right_click_label("image");
    harness.hover_label_contains("Add to new view");
    harness.click_label("2D");
    harness.set_selection_panel_opened(true);
    harness.snapshot_app("origin_image_2d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_camera_3d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("camera");
    harness.hover_label_contains("Add to new view");
    harness.click_label("3D");
    harness.snapshot_app("origin_camera_3d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_camera_2d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("camera");
    harness.hover_label_contains("Add to new view");
    harness.click_label("2D");
    harness.snapshot_app("origin_camera_2d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_world_3d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("world");
    harness.hover_label_contains("Add to new view");
    harness.click_label("3D");
    harness.snapshot_app("origin_world_3d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_world_2d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("world");
    harness.hover_label_contains("Add to new view");
    harness.click_label("2D");
    harness.snapshot_app("origin_world_2d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_root_3d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("/ (root)");
    harness.hover_label_contains("Add to new view");
    harness.click_label("3D");
    harness.snapshot_app("origin_root_3d");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_origin_root_2d() {
    let mut harness = make_test_harness();

    harness.blueprint_tree().right_click_label("3D view");
    harness.click_label("Expand all");

    harness.blueprint_tree().right_click_label("/ (root)");
    harness.hover_label_contains("Add to new view");
    harness.click_label("2D");
    harness.snapshot_app("origin_root_2d");
}
