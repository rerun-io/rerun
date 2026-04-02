//! [`Points2D`] with a fixed `ui_points` radius should appear the same size regardless of
//! how far the point is from the origin.

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::components::Radius;
use re_sdk_types::{Archetype as _, archetypes::Points2D};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView2D;
use re_viewer_context::{BlueprintContext as _, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

const SNAPSHOT_SIZE: egui::Vec2 = egui::vec2(400.0, 300.0);

#[test]
fn test_points2d_ui_radius_constant_across_positions() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    let radius = Radius::new_ui_points(4.0);

    // Using large units to make sizing issues more prominent
    let radial: Vec<[f32; 2]> = (0..=10).map(|i| [i as f32 * 500.0, 0.0]).collect();
    test_context.log_entity("radial", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points2D::new(radial).with_radii([radius]),
        )
    });

    let circle: Vec<[f32; 2]> = (0..=16)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 16.0;
            [angle.cos() * 2500.0, angle.sin() * 2500.0]
        })
        .collect();
    test_context.log_entity("circle", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points2D::new(circle).with_radii([radius]),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView2D::identifier());

        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view.id,
            ctx.store_context
                .blueprint
                .storage_engine()
                .store()
                .entity_tree(),
            re_sdk_types::blueprint::archetypes::VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(
            property_path,
            &re_sdk_types::blueprint::archetypes::VisualBounds2D::new(
                re_sdk_types::datatypes::Range2D {
                    x_range: [-500.0, 5500.0].into(),
                    y_range: [-3000.0, 3000.0].into(),
                },
            ),
        );

        blueprint.add_view_at_root(view)
    });

    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "points2d_ui_radius_constant_across_positions",
            SNAPSHOT_SIZE,
            None,
        )
        .unwrap();
}
