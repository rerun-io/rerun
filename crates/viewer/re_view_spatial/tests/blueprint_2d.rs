use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_sdk_types::{Archetype as _, archetypes};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView2D;
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewId, VisualizerConfiguration};
use re_viewport_blueprint::ViewBlueprint;

const SNAPSHOT_SIZE: egui::Vec2 = egui::vec2(400.0, 180.0);

#[test]
pub fn test_blueprint_no_overrides_or_defaults_with_spatial_2d() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    log_arrows(&mut test_context);

    let view_id = setup_blueprint(&mut test_context, None, None);
    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "blueprint_no_overrides_or_defaults_with_spatial_2d",
            SNAPSHOT_SIZE,
            None,
        )
        .unwrap();
}

#[test]
pub fn test_blueprint_overrides_with_spatial_2d() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    log_arrows(&mut test_context);

    let view_id = setup_blueprint(&mut test_context, Some(&arrow_overrides()), None);
    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "blueprint_overrides_with_spatial_2d",
            SNAPSHOT_SIZE,
            None,
        )
        .unwrap();
}

#[test]
pub fn test_blueprint_defaults_with_spatial_2d() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    log_arrows(&mut test_context);

    let view_id = setup_blueprint(&mut test_context, None, Some(&arrow_defaults()));
    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "blueprint_defaults_with_spatial_2d",
            SNAPSHOT_SIZE,
            None,
        )
        .unwrap();
}

fn log_arrows(test_context: &mut TestContext) {
    test_context.log_entity("arrows", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::Arrows2D::from_vectors([[-2.0, 1.0], [0.0, 2.0], [2.0, 1.0]])
                .with_origins([[-2.0, 0.0], [0.0, 0.0], [2.0, 0.0]]),
        )
    });
}

fn arrow_overrides() -> archetypes::Arrows2D {
    archetypes::Arrows2D::from_vectors([[-2.0, 1.0], [0.0, 2.0], [2.0, 1.0]])
        .with_origins([[-2.0, 1.5], [0.0, -0.5], [2.0, 0.75]])
        .with_labels(["BigRed", "MidGreen", "SmolBlue"])
        .with_radii([0.5, 0.25, 0.125])
        .with_colors([[255, 0, 0], [0, 255, 0], [0, 0, 255]])
}

fn arrow_defaults() -> archetypes::Arrows2D {
    archetypes::Arrows2D::update_fields()
        .with_labels(["TeenyYellow", "AverageCyan", "GigaPurple"])
        .with_radii([0.1, 0.2, 0.3])
        .with_colors([[255, 255, 0], [0, 255, 255], [255, 0, 255]])
}

fn setup_blueprint(
    test_context: &mut TestContext,
    arrow_overrides: Option<&archetypes::Arrows2D>,
    arrow_defaults: Option<&archetypes::Arrows2D>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView2D::identifier());

        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view.id,
            ctx.store_context.blueprint.tree(),
            re_sdk_types::blueprint::archetypes::VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(
            property_path.clone(),
            &re_sdk_types::blueprint::archetypes::VisualBounds2D::new(
                re_sdk_types::datatypes::Range2D {
                    x_range: [-4.0, 4.0].into(),
                    y_range: [-1.1, 2.6].into(),
                },
            ),
        );

        if let Some(arrow_overrides) = arrow_overrides {
            ctx.save_visualizers(
                &EntityPath::from("arrows"),
                view.id,
                [
                    &VisualizerConfiguration::new(archetypes::Arrows2D::visualizer())
                        .with_overrides(arrow_overrides),
                ],
            );
        }

        if let Some(arrow_defaults) = arrow_defaults {
            ctx.save_blueprint_archetype(view.defaults_path.clone(), arrow_defaults);
        }

        blueprint.add_view_at_root(view)
    })
}
