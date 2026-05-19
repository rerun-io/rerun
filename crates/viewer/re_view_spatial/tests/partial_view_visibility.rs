//! Regression test for rendering a spatial view whose allocated rect extends beyond the
//! visible window (i.e. the view is only partially visible).

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::Archetype as _;
use re_sdk_types::blueprint::archetypes::LineGrid3D;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

/// Window is `WINDOW_SIZE`, but the view gets a `max_rect` extended vertically by
/// `VERTICAL_OVERSHOOT` above and below, so the view's allocated rect vastly exceeds the
/// framebuffer in the vertical direction.
const WINDOW_SIZE: egui::Vec2 = egui::vec2(300.0, 300.0);
const VERTICAL_OVERSHOOT: f32 = 400.0;

fn set_thick_grid(ctx: &re_viewer_context::ViewerContext<'_>, view_id: ViewId) {
    let engine = ctx.store_context.blueprint.storage_engine();
    let blueprint_tree = engine.store().entity_tree();
    let property_path = re_viewport_blueprint::entity_path_for_view_property(
        view_id,
        blueprint_tree,
        LineGrid3D::name(),
    );
    ctx.save_blueprint_archetype(property_path, &LineGrid3D::new().with_stroke_width(8.0));
}

/// Run the view inside a child ui whose `max_rect` extends vertically beyond the window.
fn snapshot_partially_visible_view(
    test_context: &TestContext,
    view_id: ViewId,
    snapshot_name: &str,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(WINDOW_SIZE)
        .build_ui(|ui| {
            let window_rect = ui.max_rect();
            let extended_rect = window_rect.expand2(egui::vec2(0.0, VERTICAL_OVERSHOOT));
            ui.scope_builder(egui::UiBuilder::new().max_rect(extended_rect), |ui| {
                test_context.run_with_single_view(ui, view_id);
            });
        });

    harness.snapshot(snapshot_name);
}

fn log_checkerboard(test_context: &mut TestContext) {
    use ndarray::{Array, ShapeBuilder as _};

    let (width, height) = (64usize, 64usize);
    let cell: usize = 8;
    let mut image = Array::<u8, _>::zeros((height, width, 3).f());
    for y in 0..height {
        for x in 0..width {
            let on = ((x / cell) + (y / cell)).is_multiple_of(2);
            let v = if on { 255 } else { 0 };
            image[[y, x, 0]] = v;
            image[[y, x, 1]] = v;
            image[[y, x, 2]] = v;
        }
    }

    test_context.log_entity("checkerboard", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_sdk_types::archetypes::Image::from_color_model_and_tensor(
                re_sdk_types::datatypes::ColorModel::RGB,
                image,
            )
            .expect("valid image"),
        )
    });
}

#[test]
fn test_partial_view_visibility_2d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();
    log_checkerboard(&mut test_context);

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    snapshot_partially_visible_view(&test_context, view_id, "partial_view_visibility_2d");
}

#[test]
fn test_partial_view_visibility_3d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        set_thick_grid(ctx, view.id);
        blueprint.add_view_at_root(view)
    });

    snapshot_partially_visible_view(&test_context, view_id, "partial_view_visibility_3d");
}
