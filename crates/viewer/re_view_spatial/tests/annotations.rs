use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_annotations() {
    let mut test_context = get_test_context();

    {
        use ndarray::{Array, ShapeBuilder as _, s};

        // Log an annotation context to assign a label and color to each class
        test_context.log_entity("/", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::AnnotationContext::new([
                    (
                        0,
                        "black",
                        re_sdk_types::datatypes::Rgba32::from_rgb(0, 0, 0),
                    ),
                    (
                        1,
                        "red",
                        re_sdk_types::datatypes::Rgba32::from_rgb(255, 0, 0),
                    ),
                    (
                        2,
                        "green",
                        re_sdk_types::datatypes::Rgba32::from_rgb(0, 255, 0),
                    ),
                ]),
            )
        });

        // Log a batch of 2 rectangles with different `class_ids`
        test_context.log_entity("detections", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::Boxes2D::from_mins_and_sizes(
                    [(200.0, 50.0), (75.0, 150.0)],
                    [(30.0, 30.0), (20.0, 20.0)],
                )
                .with_class_ids([1, 2]),
            )
        });

        test_context.log_entity("segmentation/image", |builder| {
            let mut image = Array::<u8, _>::zeros((200, 300).f());
            image.slice_mut(s![50..100, 50..120]).fill(1);
            image.slice_mut(s![100..180, 130..280]).fill(2);

            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_sdk_types::archetypes::SegmentationImage::try_from(image)
                    .unwrap()
                    .with_draw_order(0.0),
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
        "annotations",
        // We need quite a bunch of pixels to be able to stack the double hover pop-ups.
        egui::vec2(300.0, 300.0) * 2.0,
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    // Make sure we can draw stuff in the hover tables.
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    // Also register the legacy UIs.
    re_data_ui::register_component_uis(&mut test_context.component_ui_registry);

    test_context
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

    {
        // There should be one view with an image and a batch of 2 rectangles.
        //
        // The image should contain a red region and a green region.
        // There should be 1 red rectangle and 1 green rectangle.

        let name = format!("{name}_overview");
        harness.run();
        harness.snapshot(&name);
    }

    {
        // Hover over each of the elements and confirm it shows the label as "red" or "green" as
        // expected.
        //
        // *Note*: when hovering the rectangles, a tooltip pertaining to the image will _also_
        // appear and indicate a label of "0". This is expected as the image is black at this
        // location.

        {
            let name = format!("{name}_hover_background");
            let raw_input = harness.input_mut();
            raw_input
                .events
                .push(egui::Event::PointerMoved((50.0, 200.0).into()));
            harness.try_run_realtime().ok();
            harness.snapshot(&name);
        }

        {
            let name = format!("{name}_hover_rect_red");
            let raw_input = harness.input_mut();
            raw_input
                .events
                .push(egui::Event::PointerMoved((200.0, 250.0).into()));
            harness.run();
            harness.snapshot(&name);
        }

        {
            let name = format!("{name}_hover_rect_green");
            let raw_input = harness.input_mut();
            raw_input
                .events
                .push(egui::Event::PointerMoved((300.0, 400.0).into()));
            harness.run();
            harness.snapshot(&name);
        }

        {
            let name = format!("{name}_hover_region_green");
            let raw_input = harness.input_mut();
            raw_input
                .events
                .push(egui::Event::PointerMoved((175.0, 450.).into()));
            harness.run();

            harness.snapshot(&name);
        }

        {
            let name = format!("{name}_hover_region_red");
            let raw_input = harness.input_mut();
            raw_input
                .events
                .push(egui::Event::PointerMoved((425., 275.0).into()));
            harness.run();
            harness.snapshot(&name);
        }
    }
}
