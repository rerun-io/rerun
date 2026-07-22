use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::blueprint::archetypes::{SpatialInformation, VisualBounds2D};
use re_sdk_types::blueprint::components::Enabled;
use re_sdk_types::{Archetype as _, archetypes::Points2D};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::{SpatialView2D, SpatialViewState};
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewStateExt as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

const SNAPSHOT_SIZE: egui::Vec2 = egui::vec2(400.0, 400.0);
const POINT_RADIUS: f32 = 0.25;

/// Renders a 2D scene with [`SpatialInformation`] blueprint options enabled (axes, bounding boxes).
#[test]
fn test_spatial_information_2d() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    test_context.log_entity("cluster", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points2D::new([
                [2.0, 2.0],
                [2.5, 3.0],
                [3.0, 2.5],
                [3.5, 3.5],
                [4.0, 2.0],
                [2.0, 4.0],
                [3.0, 4.0],
                [4.0, 4.0],
                [3.5, 2.5],
                [10.0, 10.0],
            ])
            .with_colors([[0, 122, 255]])
            .with_radii([POINT_RADIUS]),
        )
    });

    test_context.log_entity("small_cluster", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points2D::new([[6.0, 2.0], [7.0, 1.5], [8.0, 2.0], [7.0, 3.5]])
                .with_colors([[255, 128, 0]])
                .with_radii([POINT_RADIUS]),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView2D::identifier());

        let visual_bounds_path = re_viewport_blueprint::entity_path_for_view_property(
            view.id,
            ctx.store_context
                .blueprint
                .storage_engine()
                .store()
                .entity_tree(),
            VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(
            visual_bounds_path,
            &VisualBounds2D::new(re_sdk_types::datatypes::Range2D {
                x_range: [-1.0, 11.0].into(),
                y_range: [-1.0, 11.0].into(),
            }),
        );

        let spatial_information =
            ViewProperty::from_archetype_for_view::<SpatialInformation>(ctx, view.id);
        spatial_information.save_blueprint_component(
            ctx,
            &SpatialInformation::descriptor_show_axes(),
            &Enabled::from(true),
        );
        spatial_information.save_blueprint_component(
            ctx,
            &SpatialInformation::descriptor_show_bounding_box(),
            &Enabled::from(true),
        );

        blueprint.add_view_at_root(view)
    });

    {
        let mut view_states = test_context.view_states.lock();
        let state = view_states.get_mut_or_create(
            &test_context.recording_store_id,
            view_id,
            &SpatialView2D,
        );
        let state = state
            .downcast_mut::<SpatialViewState>()
            .expect("SpatialView2D should use SpatialViewState");
        state.show_smoothed_bbox = true;
        state.show_per_entity_bbox = true;
    }

    test_context
        .run_view_ui_and_save_snapshot(view_id, "spatial_information_2d", SNAPSHOT_SIZE, None)
        .unwrap();
}
