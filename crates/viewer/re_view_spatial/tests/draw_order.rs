use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_draw_order() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    {
        use ndarray::{Array, ShapeBuilder as _, s};

        // Large gray background
        test_context.log_entity("2d_layering/background", |builder| {
            let mut image = Array::<u8, _>::zeros((256, 512, 3).f());
            image.fill(64);

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Image::from_color_model_and_tensor(
                    re_sdk_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(0.0),
            )
        });

        // Smaller gradient in the middle
        test_context.log_entity("2d_layering/middle_gradient", |builder| {
            let mut image = Array::<u8, _>::zeros((256, 256, 3).f());
            image
                .slice_mut(s![.., .., 0])
                .assign(&Array::<u8, _>::from_iter(0..=255));
            image
                .slice_mut(s![.., .., 1])
                .assign(&Array::<u8, _>::from_iter((0..=255).rev()));

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Image::from_color_model_and_tensor(
                    re_sdk_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(1.0),
            )
        });

        // Slightly smaller blue in the middle, on the same layer as the previous.
        test_context.log_entity("2d_layering/middle_blue", |builder| {
            let mut image = Array::<u8, _>::zeros((192, 192, 3).f());
            image.slice_mut(s![.., .., 2]).fill(255);

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Image::from_color_model_and_tensor(
                    re_sdk_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(1.1),
            )
        });

        test_context.log_entity("2d_layering/lines_behind_rect", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::LineStrips2D::new(std::iter::once(
                    (0..20)
                        .map(|i| i as f32)
                        .map(|i| (i * 20.0, i % 2.0 * 100.0 + 70.0)),
                ))
                .with_draw_order(1.25)
                .with_colors([0xFF0000FF]),
            )
        });

        test_context.log_entity("2d_layering/rect_between_top_and_middle", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Boxes2D::from_mins_and_sizes(
                    [(64.0, 32.0)],
                    [(256.0, 128.0)],
                )
                .with_draw_order(1.5)
                .with_colors([0x000000FF]),
            )
        });

        test_context.log_entity("2d_layering/points_between_top_and_middle", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Points2D::new((0..16 * 16).map(|i| i as f32).map(|i| {
                    (
                        32.0 + (i as i32 / 16) as f32 * 16.0,
                        32.0 + (i as i32 % 16) as f32 * 16.0,
                    )
                }))
                .with_draw_order(1.51),
            )
        });

        // Small white square on top
        test_context.log_entity("2d_layering/top", |builder| {
            let mut image = Array::<u8, _>::zeros((128, 128, 3).f());
            image.fill(255);

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Image::from_color_model_and_tensor(
                    re_sdk_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(2.0),
            )
        });

        // 2D arrow sandwiched across
        test_context.log_entity("2d_layering/arrow2d_between", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Arrows2D::from_vectors([(200.0, 200.0)])
                    .with_origins([(10.0, 10.0)])
                    .with_radii([5.0])
                    .with_colors([0xFF00FFFF])
                    .with_draw_order(1.12),
            )
        });
    }

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });
    run_view_ui_and_save_snapshot(
        &test_context,
        view_id,
        "draw_order",
        egui::vec2(300.0, 150.0) * 2.0,
    );
}

fn run_view_ui_and_save_snapshot(
    test_context: &TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();
    harness.snapshot(name);
}
