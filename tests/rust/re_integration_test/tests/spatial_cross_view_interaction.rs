//! Tests cross-view spatial interaction between a pinhole image in 2D and 3D,
//! for both entity-path-derived and named transforms.

use re_integration_test::HarnessExt as _;
use re_sdk::{EntityPathFilter, TimePoint};
use re_viewer::external::re_sdk_types::{
    archetypes::{CoordinateFrame, Image, Pinhole, Points3D, Transform3D},
    blueprint::archetypes::EyeControls3D,
    components::Position3D,
    datatypes::ColorModel,
};
use re_viewer::external::re_view_spatial;
use re_viewer::external::re_viewer_context::{
    BlueprintContext as _, RecommendedView, ViewClass as _,
};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

#[tokio::test(flavor = "multi_thread")]
pub async fn test_spatial_cross_view_interaction_entity_hierarchy() {
    run_test(false);
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_spatial_cross_view_interaction_named_transforms() {
    run_test(true);
}

fn run_test(use_named_transforms: bool) {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1000.0, 600.0)),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    setup_scene(&mut harness, use_named_transforms);

    let (variant, image_origin) = if use_named_transforms {
        ("named_transforms", "image")
    } else {
        ("entity_hierarchy", "world/camera")
    };

    let root_container =
        harness.add_blueprint_container(egui_tiles::ContainerKind::Horizontal, None);

    harness.setup_viewport_blueprint(move |viewer_context, blueprint| {
        let mut view_2d = ViewBlueprint::new(
            re_view_spatial::SpatialView2D::identifier(),
            RecommendedView {
                origin: image_origin.into(),
                query_filter: EntityPathFilter::all(),
            },
        );
        view_2d.display_name = Some("Image in 2D".into());

        let mut view_3d =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        view_3d.display_name = Some("Image in 3D".into());
        let view_3d_id = view_3d.id;

        blueprint.add_views([view_2d, view_3d].into_iter(), Some(root_container), None);

        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            viewer_context.current_blueprint(),
            viewer_context.blueprint_query(),
            view_3d_id,
        );
        eye_property.save_blueprint_component(
            viewer_context,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(1.5, -1.0, 3.0),
        );
        eye_property.save_blueprint_component(
            viewer_context,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.0, 0.0, 0.5),
        );
    });

    // Hovering the image in 2D should draw the corresponding ray in the 3D view.
    let image_2d_rect = harness.get_panel_position("Image in 2D");
    hover_and_render(&mut harness, image_2d_rect.max * 0.75);
    harness.snapshot_app(&format!(
        "spatial_cross_view_interaction_{variant}_2d_to_3d"
    ));

    // Hovering the image plane in 3D should draw the corresponding position in the 2D view.
    let image_3d_rect = harness.get_panel_position("Image in 3D");
    let image_plane_position = egui::pos2(
        image_3d_rect.left() + image_3d_rect.width() * 0.7,
        image_3d_rect.top() + image_3d_rect.height() * 0.2,
    );
    hover_and_render(&mut harness, image_plane_position);
    harness.snapshot_app(&format!(
        "spatial_cross_view_interaction_{variant}_3d_to_2d"
    ));
}

fn setup_scene(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    use_named_transforms: bool,
) {
    // Don't make the image too small since the region-of-interest 2D visualization gets a bit odd for very small images.
    const IMAGE_WIDTH: usize = 128;
    const IMAGE_HEIGHT: usize = 96;

    // Simple colored matrix
    let image = ndarray::Array3::from_shape_fn((IMAGE_HEIGHT, IMAGE_WIDTH, 3), |(y, x, c)| {
        let color = match (x < IMAGE_WIDTH / 2, y < IMAGE_HEIGHT / 2) {
            (true, true) => [255, 80, 80],
            (false, true) => [80, 255, 80],
            (true, false) => [80, 80, 255],
            (false, false) => [255, 255, 80],
        };
        color[c]
    });
    let points = Points3D::new([
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ]);
    let camera_transform = Transform3D::from_translation([0.0, 0.0, 1.0]);
    let pinhole = Pinhole::from_focal_length_and_resolution(
        [100.0, 100.0],
        [IMAGE_WIDTH as f32, IMAGE_HEIGHT as f32],
    )
    .with_image_plane_distance(0.75);
    let image = Image::from_color_model_and_tensor(ColorModel::RGB, image)
        .expect("test image should be valid");

    if use_named_transforms {
        harness.log_entity("points", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &points)
                .with_archetype_auto_row(TimePoint::STATIC, &CoordinateFrame::new("world"))
        });
        harness.log_entity("camera", |builder| {
            builder
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &camera_transform
                        .with_parent_frame("world")
                        .with_child_frame("camera"),
                )
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &pinhole
                        .with_parent_frame("camera")
                        .with_child_frame("image"),
                )
        });
        harness.log_entity("image", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &image)
                .with_archetype_auto_row(TimePoint::STATIC, &CoordinateFrame::new("image"))
        });
    } else {
        harness.log_entity("world/points", |builder| {
            builder.with_archetype_auto_row(TimePoint::STATIC, &points)
        });
        harness.log_entity("world/camera", |builder| {
            builder
                .with_archetype_auto_row(TimePoint::STATIC, &camera_transform)
                .with_archetype_auto_row(TimePoint::STATIC, &pinhole)
                .with_archetype_auto_row(TimePoint::STATIC, &image)
        });
    }
}

fn hover_and_render(harness: &mut egui_kittest::Harness<'_, re_viewer::App>, position: egui::Pos2) {
    harness.hover_at(position);
    harness.run();
    harness.render().expect("app should render");
}
