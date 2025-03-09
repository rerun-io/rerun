use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_view_spatial::SpatialView3D;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId};
use re_viewport_blueprint::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_transform_hierarchy() {
    let mut test_context = get_test_context();

    let timeline_step = Timeline::new_sequence("step");

    // The Rerun logo obj's convention is y up.
    test_context.log_entity("/".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::ViewCoordinates::RIGHT_HAND_Y_UP(),
        )
    });

    {
        // Log a bunch of transforms that undo each other in total, roughly arriving at the identity transform.
        // This is done using various types of transforms, to test out that they all work as expected.

        let mut path: EntityPath = "/".into();

        path = path.join(&"translate".into());
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 1)],
                &re_types::archetypes::Transform3D::from_translation((4.0, 4.0, 4.0)),
            )
        });

        path = path.join(&"translate_back".into());
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 2)],
                &re_types::archetypes::Transform3D::from_translation((-4.0, -4.0, -4.0)),
            )
        });

        path = path.join(&"scale".into());
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 3)],
                &re_types::archetypes::Transform3D::from_scale((1.0, 0.2, 1.0)),
            )
        });

        path = path.join(&"scale_back_mat3x3".into());
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 4)],
                &re_types::archetypes::Transform3D::from_mat3x3([
                    [1.0, 0.0, 0.0],
                    [0.0, 5.0, 0.0],
                    [0.0, 0.0, 1.0],
                ]),
            )
        });

        path = path.join(&"rotate_axis_origin".into());
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 5)],
                &re_types::archetypes::Transform3D::from_rotation(
                    re_types::components::RotationAxisAngle::new(
                        (0.0, 1.0, 0.0),
                        re_types::datatypes::Angle::from_degrees(90.0),
                    ),
                ),
            )
        });

        path = path.join(&"rotate_quat".into());
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 6)],
                &re_types::archetypes::Transform3D::from_rotation(
                    // -45 degrees around the y axis.
                    // Via https://www.andre-gaschler.com/rotationconverter/
                    re_types::components::RotationQuat(re_types::datatypes::Quaternion::from_xyzw(
                        [0.0, -0.3826834, 0.0, 0.9238796],
                    )),
                ),
            )
        });

        path = path.join(&"rotate_mat3x3".into());
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 7)],
                // -45 degrees around the y axis.
                // Via https://www.andre-gaschler.com/rotationconverter/
                &re_types::archetypes::Transform3D::from_mat3x3([
                    [0.7071069, 0.0000000, -0.7071066],
                    [0.0000000, 1.0000000, 0.0000000],
                    [0.7071066, 0.0000000, 0.7071069],
                ]),
            )
        });

        // Add the Rerun asset at the end of the hierarchy.
        // (We're using a 3D model because it's easier to see the effect of arbitrary transforms here!)
        {
            let workspace_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_dir = workspace_dir
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .unwrap();
            let obj_path = workspace_dir.join("tests/assets/rerun.obj");

            path = path.join(&"asset".into());
            test_context.log_entity(path.clone(), |builder| {
                builder.with_archetype(
                    RowId::new(),
                    [(timeline_step, 0)],
                    &re_types::archetypes::Asset3D::from_file_path(&obj_path).unwrap(),
                )
            });
        }
    }

    let view_id = setup_blueprint(&mut test_context);

    run_view_ui_and_save_snapshot(
        &mut test_context,
        timeline_step,
        view_id,
        "transform_hierarchy",
        egui::vec2(300.0, 150.0),
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    test_context
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView::root(),
        );

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        view_id
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    timeline: Timeline,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    test_context.set_active_timeline(timeline);

    let rec_cfg = test_context.recording_config.clone();

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    let view_class = ctx
                        .view_class_registry()
                        .get_class_or_log_error(SpatialView3D::identifier());

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
                        .expect("failed to run view ui");
                });

                test_context.handle_system_commands();
            });
        });

    {
        // This test adds more and more transforms in a hierarchy on each step on the `steps` timeline.
        //
        // What you should see on each step on the `steps` timeline:
        // * 0: There's a Rerun logo is at the origin, the `e` sits roughly above the origin.
        // * 1: The logo is translated a few units diagonally positively on x/y/z.
        // * 2: The logo is back to normal (frame 0) again.
        // * 3: The logo is squished along its height
        // * 4: The logo is back to normal (frame 0) again.
        // * 5: The logo rotated 90 degrees around the y axis (green). It reads now along the z axis (blue).
        // * 6: The logo rotated 45 degrees around the y axis (green).
        // * 7: The logo is back to normal (frame 0) again.

        let raw_input = harness.input_mut();
        raw_input
            .events
            .push(egui::Event::PointerMoved((100.0, 100.0).into()));
        raw_input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::Vec2::UP * 3.0,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(8);

        for time in 0..=7 {
            let name = format!("{name}_{}_{time}", timeline.name());

            rec_cfg
                .time_ctrl
                .write()
                .set_time_for_timeline(timeline, time);

            harness.run_steps(8);

            let broken_percent_threshold = 0.0036;
            let num_pixels = (size.x * size.y).ceil() as u64;

            use re_viewer_context::test_context::HarnessExt as _;
            harness.snapshot_with_broken_pixels_threshold(
                &name,
                num_pixels,
                broken_percent_threshold,
            );
        }
    }
}
