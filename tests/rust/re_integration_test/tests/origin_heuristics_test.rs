// &archetypes::Image::from_color_model_and_tensor(
//                             re_types::datatypes::ColorModel::RGB,
//                             Array::<u8, _>::zeros((h, w, 3).f()),
//                         )

use egui::Modifiers;

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_image() -> re_types::archetypes::Image {
    let image = ndarray::Array3::from_shape_fn((256, 256, 3), |(y, x, c)| match c {
        0 => x as u8,
        1 => (x ^ y) as u8,
        2 => y as u8,
        _ => unreachable!(),
    });

    re_types::archetypes::Image::from_color_model_and_tensor(
        re_types::datatypes::ColorModel::RGB,
        image,
    )
}

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_selection_panel_opened(false);

    // Log some data
    harness.log_entity("/", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(0.0, 0.0, 0.0)],
                [(1.0, 1.0, 1.0)],
            )
            .with_colors([0xFF0000FF]),
        )
    });

    harness.log(
        "/world",
        re_types::components::ViewCoordinates::RightHandYDown,
        true,
    );

    harness.log(
        "/world/camera",
        re_types::components::Pinhole::new([10, 10], [4.0, 4.0], [5.0, 5.0]),
        true,
    );

    harness.log_entity("world/camera", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Transform3D::from_rotation(
                re_types::components::RotationAxisAngle::new(
                    [0.0, 0.0, 1.0],
                    std::f32::consts::PI / 2.0,
                ),
            ),
        )
    });

    harness.log_entity("world/camera/image", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::default(), &make_test_image())
    });

    harness.log_entity("world/camera/image/keypoint", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Points2D::from_points_and_radii(
                [(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)],
                [0.5, 0.5, 0.5],
            ),
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
pub async fn test_foo() {
    let mut harness = make_test_harness();

    harness.snapshot_app("xtemp");

    //     // Test context menus of view panel title widgets
    //     harness.right_click_nth_label("3D view", 1);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_02");
    //     harness.key_press(egui::Key::Escape);

    //     harness.right_click_nth_label("2D view", 1);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_03");
    //     harness.key_press(egui::Key::Escape);

    //     // Test context menus of view items in the blueprint panel
    //     harness.right_click_nth_label("3D view", 0);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_04");
    //     harness.key_press(egui::Key::Escape);

    //     harness.right_click_nth_label("2D view", 0);
    //     harness.hover_label_contains("Move to new container");
    //     harness.snapshot_app("context_menu_invalid_sub_container_05");
    //     harness.key_press(egui::Key::Escape);
}
