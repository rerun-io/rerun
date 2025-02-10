use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_view_spatial::SpatialView2D;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass, ViewId};
use re_viewport_blueprint::test_context_ext::TestContextExt;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_draw_order() {
    let mut test_context = get_test_context();

    {
        use ndarray::{s, Array, ShapeBuilder};

        // Large gray background
        test_context.log_entity("2d_layering/background".into(), |builder| {
            let mut image = Array::<u8, _>::zeros((256, 512, 3).f());
            image.fill(64);

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Image::from_color_model_and_tensor(
                    re_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(0.0),
            )
        });

        // Smaller gradient in the middle
        test_context.log_entity("2d_layering/middle_gradient".into(), |builder| {
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
                &re_types::archetypes::Image::from_color_model_and_tensor(
                    re_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(1.0),
            )
        });

        // Slightly smaller blue in the middle, on the same layer as the previous.
        test_context.log_entity("2d_layering/middle_blue".into(), |builder| {
            let mut image = Array::<u8, _>::zeros((192, 192, 3).f());
            image.slice_mut(s![.., .., 2]).fill(255);

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Image::from_color_model_and_tensor(
                    re_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(1.1),
            )
        });

        test_context.log_entity("2d_layering/lines_behind_rect".into(), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::LineStrips2D::new(std::iter::once(
                    (0..20)
                        .map(|i| i as f32)
                        .map(|i| (i * 20.0, i % 2.0 * 100.0 + 70.0)),
                ))
                .with_draw_order(1.25)
                .with_colors([0xFF0000FF]),
            )
        });

        test_context.log_entity(
            "2d_layering/rect_between_top_and_middle".into(),
            |builder| {
                builder.with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Boxes2D::from_mins_and_sizes(
                        [(64.0, 32.0)],
                        [(256.0, 128.0)],
                    )
                    .with_draw_order(1.5)
                    .with_colors([0x000000FF]),
                )
            },
        );

        test_context.log_entity(
            "2d_layering/points_between_top_and_middle".into(),
            |builder| {
                builder.with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Points2D::new((0..16 * 16).map(|i| i as f32).map(|i| {
                        (
                            32.0 + (i as i32 / 16) as f32 * 16.0,
                            32.0 + (i as i32 % 16) as f32 * 16.0,
                        )
                    }))
                    .with_draw_order(1.51),
                )
            },
        );

        // Small white square on top
        test_context.log_entity("2d_layering/top".into(), |builder| {
            let mut image = Array::<u8, _>::zeros((128, 128, 3).f());
            image.fill(255);

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Image::from_color_model_and_tensor(
                    re_types::datatypes::ColorModel::RGB,
                    image,
                )
                .unwrap()
                .with_draw_order(2.0),
            )
        });
    }

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "draw_order",
        egui::vec2(300.0, 150.0) * 2.0,
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView2D>();

    test_context
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView2D::identifier(),
            RecommendedView::root(),
        );

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
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    let view_class = ctx
                        .view_class_registry
                        .get_class_or_log_error(SpatialView2D::identifier());

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
    harness.snapshot(name);
}
