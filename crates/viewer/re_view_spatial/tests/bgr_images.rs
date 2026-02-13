#![expect(clippy::unwrap_used)] // It's a test!

use image::GenericImageView as _;
use itertools::Itertools as _;
use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::Archetype as _;
use re_sdk_types::blueprint::components::BackgroundKind;
use re_sdk_types::datatypes::ColorModel;
use re_sdk_types::image::ImageChannelType;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

fn convert_pixels_to<T: From<u8> + Copy>(u8s: &[u8]) -> Vec<T> {
    u8s.iter().map(|u| T::from(*u)).collect()
}

fn run_bgr_test<T: ImageChannelType>(
    image: &[T],
    size: [u32; 2],
    color_model: ColorModel,
    snapshot_results: &mut SnapshotResults,
) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    test_context.log_entity("bgr_image", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Image::from_elements(image, size, color_model),
        )
    });

    // Set up a view with a purple-ish background to catch alpha blending issues
    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view.id,
            ctx.store_context.blueprint.tree(),
            re_sdk_types::blueprint::archetypes::Background::name(),
        );
        ctx.save_blueprint_archetype(
            property_path.clone(),
            &re_sdk_types::blueprint::archetypes::Background::new(BackgroundKind::SolidColor)
                .with_color(re_sdk_types::components::Color::from_rgb(200, 100, 200)),
        );
        blueprint.add_view_at_root(view)
    });

    // type_name of half is "half::binary16::f16"
    let pixel_type_name = std::any::type_name::<T>().split("::").last().unwrap();

    let snapshot_name = format!(
        "bgr_images_{}_{pixel_type_name}",
        color_model.to_string().to_lowercase(),
    );

    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        &snapshot_name,
        egui::vec2(300.0, 200.0),
        None,
    ));
}

fn run_all_formats(
    image: &[u8],
    size: [u32; 2],
    color_model: ColorModel,
    snapshot_results: &mut SnapshotResults,
) {
    run_bgr_test(image, size, color_model, snapshot_results);
    run_bgr_test(
        &convert_pixels_to::<u16>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<u32>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<u64>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<i16>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<i32>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<i64>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<half::f16>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<f32>(image),
        size,
        color_model,
        snapshot_results,
    );
    run_bgr_test(
        &convert_pixels_to::<f64>(image),
        size,
        color_model,
        snapshot_results,
    );
}

#[test]
fn test_bgr_images() {
    let mut snapshot_results = SnapshotResults::new();
    let test_image =
        image::load_from_memory(include_bytes!("../../../../tests/assets/image/grinda.jpg"))
            .unwrap();
    let size = test_image.dimensions().into();
    let rgb_u8 = test_image.to_rgb8().into_raw();

    let bgr_u8 = rgb_u8
        .chunks(3)
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect_vec();
    run_all_formats(&bgr_u8, size, ColorModel::BGR, &mut snapshot_results);

    let bgra_u8 = rgb_u8
        .chunks(3)
        .flat_map(|p| [p[2], p[1], p[0], 255])
        .collect_vec();
    run_all_formats(&bgra_u8, size, ColorModel::BGRA, &mut snapshot_results);
}
