//! Renders a model loaded through `re_importer::MjcfTree`, to cover what the MJCF importer logs
//! (geoms, colors and the frame of every body) together with the visualizers that show it.

use std::path::Path;

use re_importer::{KinematicTree as _, MjcfTree};
use re_log_types::{TimeInt, TimePoint, Timeline};
use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::blueprint::archetypes::{EyeControls3D, LineGrid3D};
use re_sdk_types::components::Position3D;
use re_sdk_types::components::Visible;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// One geom of every supported type, on bodies that are offset and rotated relative to their parent.
const MODEL: &str = r#"
    <mujoco model="shapes">
      <compiler angle="radian"/>
      <asset>
        <material name="blue" rgba="0.2 0.4 1 1"/>
        <mesh name="tetrahedron" vertex="0 0 0  0.3 0 0  0 0.3 0  0 0 0.3" face="0 2 1  0 1 3  0 3 2  1 2 3"/>
      </asset>
      <worldbody>
        <geom type="plane" size="1.5 1 0.01" rgba="0.8 0.8 0.8 1"/>
        <body name="base" pos="-1 0 0.2">
          <geom type="box" size="0.2 0.2 0.2" rgba="1 0 0 1"/>
          <body name="arm" pos="0 0 0.2" euler="0 0.5 0">
            <joint name="yaw" axis="0 0 1"/>
            <joint name="pitch" axis="0 1 0"/>
            <geom type="capsule" size="0.05" fromto="0 0 0 0 0 0.6" rgba="0 1 0 1"/>
            <body name="hand" pos="0 0 0.6">
              <geom type="sphere" size="0.1" material="blue"/>
            </body>
          </body>
        </body>
        <body name="shapes" pos="0.5 0 0.3">
          <geom type="cylinder" size="0.1 0.2" pos="0 -0.5 0" rgba="1 1 0 1"/>
          <geom type="ellipsoid" size="0.1 0.2 0.3" rgba="1 0 1 1"/>
          <geom type="mesh" mesh="tetrahedron" pos="0 0.5 0" rgba="0 1 1 1"/>
        </body>
      </worldbody>
    </mujoco>"#;

#[test]
pub fn test_mjcf_model() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let tree =
        MjcfTree::from_xml(MODEL, Path::new("."), "fallback", None).expect("the model is valid");
    let mut chunks = Vec::new();
    tree.emit(&mut |chunk| chunks.push(chunk), &TimePoint::STATIC, true)
        .expect("the model has no external files that could fail to load");
    test_context.add_chunks(chunks.into_iter());

    // Moving both joints of `arm` exercises its intermediate `arm::yaw` frame.
    // Yawing by π and then pitching by 0.5 undoes the arm's `euler` tilt, so it stands upright.
    let timeline = Timeline::new_sequence("frame");
    test_context.set_active_timeline(*timeline.name());
    let posed = TimePoint::STATIC.with(timeline, TimeInt::from_sequence(1.try_into().unwrap()));
    let transforms = tree
        .compute_joint_transforms(&[("yaw", std::f64::consts::PI), ("pitch", 0.5)], false)
        .expect("both joints are hinges");
    test_context.log_entity("transforms", |mut builder| {
        for transform in transforms {
            builder = builder.with_archetype_auto_row(posed.clone(), &Transform3D::from(transform));
        }
        builder
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_id = blueprint.add_view_at_root(ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                // The model's root entity is in the world frame, which all body frames connect to.
                origin: "/shapes".into(),
                query_filter: "+ /**".parse().expect("valid query filter"),
            },
        ));

        // A fixed eye, so that it does not follow the scene bounds when the arm moves.
        let property = ViewProperty::from_archetype_for_view::<EyeControls3D>(ctx, view_id);
        property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(0.6, -2.6, 1.6),
        );
        property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(-0.3, 0.0, 0.4),
        );

        // The grid is coplanar with the model's plane geom, and which of the two wins the depth test
        // differs between graphics backends.
        let property = ViewProperty::from_archetype_for_view::<LineGrid3D>(ctx, view_id);
        property.save_blueprint_component(
            ctx,
            &LineGrid3D::descriptor_visible(),
            &Visible::from(false),
        );

        view_id
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(400.0, 400.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    let mut snapshot_results = SnapshotResults::new();

    test_context.set_time(0);
    harness.run();
    snapshot_results.add(harness.try_snapshot("mjcf_model"));

    test_context.set_time(1);
    harness.run();
    snapshot_results.add(harness.try_snapshot("mjcf_model_posed"));
}
