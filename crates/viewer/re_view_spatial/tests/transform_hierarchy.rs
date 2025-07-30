use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_test_context::{TestContext, external::egui_kittest::SnapshotOptions};
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_transform_hierarchy() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let timeline_step = Timeline::new_sequence("step");

    // The Rerun logo obj's convention is y up.
    test_context.log_entity("/", |builder| {
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

        path = path / "translate";
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 1)],
                &re_types::archetypes::Transform3D::from_translation((4.0, 4.0, 4.0)),
            )
        });

        path = path / "translate_back";
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 2)],
                &re_types::archetypes::Transform3D::from_translation((-4.0, -4.0, -4.0)),
            )
        });

        path = path / "scale";
        test_context.log_entity(path.clone(), |builder| {
            builder.with_archetype(
                RowId::new(),
                [(timeline_step, 3)],
                &re_types::archetypes::Transform3D::from_scale((1.0, 0.2, 1.0)),
            )
        });

        path = path / "scale_back_mat3x3";
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

        path = path / "rotate_axis_origin";
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

        path = path / "rotate_quat";
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

        path = path / "rotate_mat3x3";
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

            path = path / "asset";
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

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());

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
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
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

        let mut success = true;
        for time in 0..=7 {
            let name = format!("{name}_{}_{time}", timeline.name());

            rec_cfg
                .time_ctrl
                .write()
                .set_time_for_timeline(timeline, time);

            harness.run_steps(8);

            let broken_pixels_fraction = 0.004;

            let options = SnapshotOptions::new().failed_pixel_count_threshold(
                (size.x * size.y * broken_pixels_fraction).round() as usize,
            );

            if harness.try_snapshot_options(&name, &options).is_err() {
                success = false;
            }
        }
        assert!(success, "one or more snapshots failed");
    }
}
