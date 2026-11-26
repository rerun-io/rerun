use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::blueprint::archetypes::{Rulers, VisualBounds2D};
use re_sdk_types::components::{Name, Visible};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView2D;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Renders a 2D scene with the rulers shown and custom axis names.
#[test]
fn test_rulers() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    test_context.log_entity("points", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points2D::new([[0.0, 0.0], [10.0, 0.0], [0.0, 5.0], [10.0, 5.0]])
                .with_colors([[0, 122, 255]])
                .with_radii([0.25]),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView2D::identifier());

        let visual_bounds = ViewProperty::from_archetype_for_view::<VisualBounds2D>(ctx, view.id);
        visual_bounds.save_blueprint_component(
            ctx,
            &VisualBounds2D::descriptor_range(),
            &re_sdk_types::blueprint::components::VisualBounds2D::from(
                re_sdk_types::encodings::Range2D {
                    x_range: [-1.0, 11.0].into(),
                    y_range: [-1.0, 6.0].into(),
                },
            ),
        );

        let rulers = ViewProperty::from_archetype_for_view::<Rulers>(ctx, view.id);
        rulers.save_blueprint_component(ctx, &Rulers::descriptor_visible(), &Visible::from(true));
        rulers.save_blueprint_component(ctx, &Rulers::descriptor_name_x(), &Name::from("u [px]"));
        rulers.save_blueprint_component(ctx, &Rulers::descriptor_name_y(), &Name::from("v [px]"));

        blueprint.add_view_at_root(view)
    });

    test_context
        .run_view_ui_and_save_renderer_snapshot(view_id, "rulers", egui::vec2(400.0, 300.0), None)
        .unwrap();
}
