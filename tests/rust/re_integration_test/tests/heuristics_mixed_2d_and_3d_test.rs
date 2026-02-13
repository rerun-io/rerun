use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_sdk_types;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

const IMAGE_SIZE: (usize, usize) = (20, 30);

fn log_test_image(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    entity_path: &str,
    color: [u8; 3],
) {
    let image =
        ndarray::Array3::from_shape_fn((IMAGE_SIZE.0, IMAGE_SIZE.1, 3), |(_, _, c)| color[c]);

    harness.log_entity(entity_path, |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Image::from_color_model_and_tensor(
                re_sdk_types::datatypes::ColorModel::RGB,
                image,
            )
            .expect("Failed to create image"),
        )
    });
}

fn make_multi_view_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();

    // Log some data
    harness.log_entity("3D", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::ViewCoordinates::RIGHT_HAND_Y_DOWN(),
        )
    });

    harness.log_entity("3D/box", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(0.0, 0.0, 0.0)],
                [(0.1, 0.1, 0.1)],
            )
            .with_colors([0xFF9001FF]),
        )
    });
    harness.log_entity("3D/camera", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Pinhole::from_focal_length_and_resolution(
                [128.0, 128.0],
                [IMAGE_SIZE.1 as f32, IMAGE_SIZE.0 as f32],
            )
            .with_principal_point([IMAGE_SIZE.0 as f32 / 2.0, IMAGE_SIZE.1 as f32 / 2.0])
            .with_image_plane_distance(1.0),
        )
    });
    log_test_image(&mut harness, "3D/camera", [0, 0, 255]);
    log_test_image(&mut harness, "image1", [255, 0, 0]);
    log_test_image(&mut harness, "image2", [0, 255, 0]);

    harness.set_selection_panel_opened(false);
    harness
}

// Tests whether blueprint heuristics work correctly when mixing 2D and 3D data.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_heuristics_mixed_2d_and_3d() {
    let mut harness = make_multi_view_test_harness();

    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.set_auto_layout(true, _viewer_context);
        blueprint.set_auto_views(true, _viewer_context);
    });

    // Need to step because view coordinates aren't applied in the first frame.
    harness.run_steps(2);

    harness.snapshot_app("heuristics_mixed_2d_and_3d");
}
