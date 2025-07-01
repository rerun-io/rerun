use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_view_spatial::{SpatialView3D, SpatialViewState};
use re_viewer_context::{
    RecommendedView, ViewClass as _, ViewId,
    test_context::{HarnessExt as _, TestContext},
};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

// This test is very similar to the transform_hierarchy snippet!
// We're testing different origins and see if we get the expected results.
#[test]
pub fn test_transform_tree_origins() {
    let mut test_context = get_test_context();

    {
        test_context.log_entity("/", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::ViewCoordinates::RIGHT_HAND_Z_UP(),
            )
        });

        // Setup points, all are in the center of their own space:
        test_context.log_entity("sun", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Points3D::new([[0.0, 0.0, 0.0]])
                    .with_radii([0.6])
                    .with_colors([0xFFC800FF]),
            )
        });

        test_context.log_entity("sun/planet", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Points3D::new([[0.0, 0.0, 0.0]])
                    .with_radii([0.4]) // Yes it's a big planet ;-)
                    .with_colors([0x2850C8FF]),
            )
        });

        // Add a bunch of small cubes around the planet, to test that poses are handled correctly.
        test_context.log_entity("sun/planet/cuberoids", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Boxes3D::from_half_sizes([[0.1, 0.1, 0.1]])
                        .with_colors([0x6495EDFF]) // cornflower blue
                        .with_fill_mode(re_types::components::FillMode::Solid),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::InstancePoses3D::new().with_translations(
                        (0..6).flat_map(|x| {
                            (0..6).map(move |y| [x as f32 - 3.0, y as f32 - 3.0, 0.0])
                        }),
                    ),
                )
        });

        test_context.log_entity("sun/planet/moon", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Points3D::new([[0.0, 0.0, 0.0]])
                    .with_radii([0.15])
                    .with_colors([0xB4B4B4FF]),
            )
        });

        // Draw fixed paths where the planet & moon would move.
        let d_planet = 6.0_f32;
        let d_moon = 3.0_f32;
        let angles = (0..=100).map(|i| i as f32 * 0.01 * std::f32::consts::TAU);
        let circle: Vec<_> = angles.map(|angle| [angle.sin(), angle.cos()]).collect();

        test_context.log_entity("sun/planet_path", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::LineStrips3D::new([
                    re_types::components::LineStrip3D::from_iter(
                        circle
                            .iter()
                            .map(|p| [p[0] * d_planet, p[1] * d_planet, 0.0]),
                    ),
                ]),
            )
        });

        test_context.log_entity("sun/planet/moon_path", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::LineStrips3D::new([
                    re_types::components::LineStrip3D::from_iter(
                        circle.iter().map(|p| [p[0] * d_moon, p[1] * d_moon, 0.0]),
                    ),
                ]),
            )
        });

        // Place planet and moon. (Unlike the snippet, we're not animating this.)
        let r_moon = 5.0_f32;
        let r_planet = 2.0_f32;

        test_context.log_entity("sun/planet", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Transform3D::from_translation_rotation(
                    [r_planet.sin() * d_planet, r_planet.cos() * d_planet, 0.0],
                    re_types::datatypes::RotationAxisAngle {
                        axis: [1.0, 0.0, 0.0].into(),
                        angle: re_types::datatypes::Angle::from_degrees(20.0),
                    },
                ),
            )
        });

        test_context.log_entity("sun/planet/moon", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Transform3D::from_translation_rotation(
                    [r_moon.cos() * d_moon, r_moon.sin() * d_moon, 0.0],
                    // This rotation only really has a visual effect when we put the origin to the moon. Which we're going to do!
                    re_types::datatypes::RotationAxisAngle {
                        axis: [0.0, 0.0, 1.0].into(),
                        angle: re_types::datatypes::Angle::from_degrees(20.0),
                    },
                )
                .with_relation(re_types::components::TransformRelation::ChildFromParent),
            )
        });
    }

    for origin in ["/sun", "/sun/planet", "/sun/planet/moon"] {
        let view_id = setup_blueprint(&mut test_context, origin);
        run_view_ui_and_save_snapshot(
            &mut test_context,
            view_id,
            &format!("transform_tree_origins_{}", origin.replace('/', "_")),
            egui::vec2(400.0, 250.0),
            500,
        );
    }
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    // Make sure we can draw stuff in the hover tables.
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    // Also register the legacy UIs.
    re_data_ui::register_component_uis(&mut test_context.component_ui_registry);

    test_context
}

fn setup_blueprint(test_context: &mut TestContext, origin: &str) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: origin.into(),
                query_filter: "+ /**".parse().expect("valid query filter"),
            },
        ))
    })
}

#[track_caller]
fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
    num_pixels_allowed_to_deviate: usize,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    // Show axis at the origin to help with orientation.
                    // TODO(#10028): this should be a blueprint property, not a state.
                    {
                        let view_class = ctx
                            .view_class_registry()
                            .get_class_or_log_error(SpatialView3D::identifier());
                        let mut view_states = test_context.view_states.lock();
                        let view_state = view_states.get_mut_or_create(view_id, view_class);
                        view_state
                            .as_any_mut()
                            .downcast_mut::<SpatialViewState>()
                            .expect("view state is not of correct type")
                            .state_3d
                            .show_axes = true;
                    }

                    test_context.ui_for_single_view(ui, ctx, view_id);
                });

                test_context.handle_system_commands();
            });
        });

    harness.snapshot_with_broken_pixels(name, num_pixels_allowed_to_deviate);
}
