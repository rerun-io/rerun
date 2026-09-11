use re_integration_test::HarnessExt as _;
use re_integration_test::ViewerHarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_log_types::EntityPath;
use re_viewer::external::re_sdk_types::Archetype as _;
use re_viewer::external::re_viewer_context::{BlueprintContext as _, Item, ViewClass as _};
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::{ViewBlueprint, entity_path_for_view_property};

fn selected_item(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) -> Option<Item> {
    harness.run_with_app_context(|ctx| ctx.selection().single_item().cloned())
}

fn hovered_item(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) -> Option<Item> {
    harness.run_with_app_context(|ctx| ctx.hovered().single_item().cloned())
}

/// Number of rendered frames we give the picking readback to report the hovered item.
const MAX_PICKING_FRAMES: usize = 10;

fn is_entity(item: Option<&Item>, expected: &EntityPath) -> bool {
    item.and_then(Item::entity_path)
        .is_some_and(|path| path == expected)
}

fn assert_item_entity_path(item: Option<&Item>, expected: &EntityPath, interaction: &str) {
    assert!(
        is_entity(item, expected),
        "{interaction} should target {expected}, got {item:?}"
    );
}

/// Moves the pointer to `position` and renders frames until `is_expected` accepts the hovered item.
///
/// Picking in the spatial views goes through an asynchronous GPU readback, so the hovered item
/// only becomes available a few rendered frames after the pointer moved. Returns the last
/// observed hovered item so the caller can assert on it (and get a useful message on failure).
fn hover_until(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    position: egui::Pos2,
    is_expected: impl Fn(Option<&Item>) -> bool,
) -> Option<Item> {
    harness.hover_at(position);
    harness.run_ok();
    let mut hovered = None;
    for _ in 0..MAX_PICKING_FRAMES {
        harness.render().expect("the spatial view should render");
        harness.run_steps(1);
        hovered = hovered_item(harness);
        if is_expected(hovered.as_ref()) {
            break;
        }
    }
    hovered
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_spatial_view_hover_and_selection() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1200.0, 700.0)),
        max_steps: Some(100),
        snapshot_test_options: re_ui::testing::TestOptions::Rendering3D,
        ..Default::default()
    });
    // For the hover picking gpu readback to work right, we need to render every frame
    harness.set_render_every_step(true);
    harness.init_recording();
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    harness.log_entity("points2d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Points2D::new([[0.0, 0.0]])
                .with_radii([re_sdk_types::components::Radius::new_ui_points(24.0)]),
        )
    });
    harness.log_entity("points3d", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Points3D::new([[0.0, 0.0, 0.0]])
                .with_radii([re_sdk_types::components::Radius::new_ui_points(48.0)]),
        )
    });

    let (view_2d_id, view_3d_id) = harness.setup_viewport_blueprint(|ctx, blueprint| {
        let mut view_2d =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
        view_2d.display_name = Some("2D interaction view".into());
        let view_2d_id = view_2d.id;

        let bounds_path = entity_path_for_view_property(
            view_2d_id,
            ctx.store_context
                .blueprint
                .storage_engine()
                .store()
                .entity_tree(),
            re_sdk_types::blueprint::archetypes::VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(
            bounds_path,
            &re_sdk_types::blueprint::archetypes::VisualBounds2D::new(
                re_sdk_types::datatypes::Range2D {
                    x_range: [-2.0, 2.0].into(),
                    y_range: [-2.0, 2.0].into(),
                },
            ),
        );

        let mut view_3d =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        view_3d.display_name = Some("3D interaction view".into());
        let view_3d_id = view_3d.id;

        blueprint.add_views([view_2d, view_3d].into_iter(), None, None);
        (view_2d_id, view_3d_id)
    });

    let points2d = EntityPath::from("points2d");
    let points3d = EntityPath::from("points3d");

    // Both points sit at the origin: the 2D view has its visual bounds pinned to a range
    // centered on it, and the default 3D eye looks at it, so the center of each view is
    // where the point is.
    let view_2d_rect = harness.get_panel_position("2D interaction view");
    let point_2d = view_2d_rect.center();
    let hovered = hover_until(&mut harness, point_2d, |item| is_entity(item, &points2d));
    assert_item_entity_path(hovered.as_ref(), &points2d, "2D hover");
    harness.click_at(point_2d);
    let selected = selected_item(&mut harness);
    assert_item_entity_path(selected.as_ref(), &points2d, "2D selection");

    let background_2d = view_2d_rect.center() + egui::vec2(100.0, 100.0);
    let hovered = hover_until(&mut harness, background_2d, |item| {
        item == Some(&Item::View(view_2d_id))
    });
    assert_eq!(hovered, Some(Item::View(view_2d_id)));
    harness.click_at(background_2d);
    assert_eq!(selected_item(&mut harness), Some(Item::View(view_2d_id)));

    let view_3d_rect = harness.get_panel_position("3D interaction view");
    let point_3d = view_3d_rect.center();
    let hovered = hover_until(&mut harness, point_3d, |item| is_entity(item, &points3d));
    assert_item_entity_path(hovered.as_ref(), &points3d, "3D hover");
    harness.click_at(point_3d);
    let selected = selected_item(&mut harness);
    assert_item_entity_path(selected.as_ref(), &points3d, "3D selection");

    let background_3d = view_3d_rect.center() + egui::vec2(100.0, 100.0);
    let hovered = hover_until(&mut harness, background_3d, |item| {
        item == Some(&Item::View(view_3d_id))
    });
    assert_eq!(hovered, Some(Item::View(view_3d_id)));
    harness.click_at(background_3d);
    assert_eq!(selected_item(&mut harness), Some(Item::View(view_3d_id)));
}
