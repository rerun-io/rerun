#![expect(clippy::unwrap_used)] // It's a test!

use image::GenericImageView as _;
use itertools::Itertools as _;
use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_test_context::{
    TestContext,
    external::egui_kittest::{OsThreshold, SnapshotOptions},
};
use re_test_viewport::TestContextExt as _;
use re_types::{
    Archetype as _, blueprint::components::BackgroundKind, datatypes::ColorModel,
    image::ImageChannelType,
};
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn convert_pixels_to<T: From<u8> + Copy>(u8s: &[u8]) -> Vec<T> {
    u8s.iter().map(|u| T::from(*u)).collect()
}

fn run_bgr_test<T: ImageChannelType>(image: &[T], size: [u32; 2], color_model: ColorModel) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    test_context.log_entity("bgr_image", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Image::from_elements(image, size, color_model),
        )
    });

    // Set up a view with a purple-ish background to catch alpha blending issues
    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view.id,
            ctx.store_context.blueprint.tree(),
            re_types::blueprint::archetypes::Background::name(),
        );
        ctx.save_blueprint_archetype(
            property_path.clone(),
            &re_types::blueprint::archetypes::Background::new(BackgroundKind::SolidColor)
                .with_color(re_types::components::Color::from_rgb(200, 100, 200)),
        );
        blueprint.add_view_at_root(view)
    });

    // type_name of half is "half::binary16::f16"
    let pixel_type_name = std::any::type_name::<T>().split("::").last().unwrap();

    let snapshot_name = format!(
        "bgr_images_{}_{pixel_type_name}",
        color_model.to_string().to_lowercase(),
    );

    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        &snapshot_name,
        egui::vec2(160.0, 120.0),
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
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    harness.snapshot_options(
        name,
        &SnapshotOptions::new().failed_pixel_count_threshold(OsThreshold::new(2)),
    );
}

fn run_all_formats(image: &[u8], size: [u32; 2], color_model: ColorModel) {
    run_bgr_test(image, size, color_model);
    run_bgr_test(&convert_pixels_to::<u16>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<u32>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<u64>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<i16>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<i32>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<i64>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<half::f16>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<f32>(image), size, color_model);
    run_bgr_test(&convert_pixels_to::<f64>(image), size, color_model);
}

#[test]
fn test_bgr_images() {
    let test_image =
        image::load_from_memory(include_bytes!("../../../../tests/assets/image/grinda.jpg"))
            .unwrap();
    let size = test_image.dimensions().into();
    let rgb_u8 = test_image.to_rgb8().into_raw();

    let bgr_u8 = rgb_u8
        .chunks(3)
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect_vec();
    run_all_formats(&bgr_u8, size, ColorModel::BGR);

    let bgra_u8 = rgb_u8
        .chunks(3)
        .flat_map(|p| [p[2], p[1], p[0], 255])
        .collect_vec();
    run_all_formats(&bgra_u8, size, ColorModel::BGRA);
}
