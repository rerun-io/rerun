//! Tests for [`re_context_menu::set_entity_visibility_in_all_views`].

use re_chunk::TimePoint;
use re_context_menu::{set_entity_visibility_in_all_views, set_entity_visibility_in_view};
use re_entity_db::EntityPath;
use re_log_types::EntityPathFilter;
use re_log_types::example_components::{MyPoint, MyPoints};
use re_sdk_types::blueprint::archetypes::EntityBehavior;
use re_sdk_types::components::Visible;
use re_test_context::TestContext;
use re_test_viewport::{TestContextExt as _, TestView};
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId, ViewerContext};
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

/// Happy path: hiding `/world/points` in 3 wildcard views writes a
/// `visible=Some(false)` override to each.
#[test]
fn writes_visible_override_to_each_containing_view() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let entity_path = EntityPath::from("/world/points");
    log_point(&mut test_context, &entity_path);
    let view_ids = three_wildcard_views(&mut test_context);

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &entity_path, false);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            assert_eq!(read_override(ctx, view_id, &entity_path), Some(false));
        }
    });
}

/// Mixed pre-state: 2 of 3 views explicitly hide the entity, the 3rd inherits
/// `visible=true`. After `set(.., true)`, the 2 overrides should clear AND
/// the 3rd must stay untouched — writing a fresh `Some(true)` there would
/// silently sever parent-to-child visibility inheritance.
#[test]
fn show_clears_existing_and_skips_inheriting_views() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let entity_path = EntityPath::from("/world/points");
    log_point(&mut test_context, &entity_path);
    let view_ids = three_wildcard_views(&mut test_context);

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in [view_ids[0], view_ids[1]] {
            set_entity_visibility_in_view(ctx, view_id, &entity_path, false);
        }
    });
    test_context.handle_system_commands(&egui::Context::default());
    refresh_query_results(&mut test_context);

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &entity_path, true);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            assert_eq!(read_override(ctx, view_id, &entity_path), None);
        }
    });
}

/// 3 views, the 3rd filtered to match only `/world/points/red`, so
/// `/world/points` exists in it as a synthesized prefix-only ancestor. The
/// helper must skip it — writing an override at `/world/points` there would
/// cascade to `/world/points/red`, hiding an entity the user never targeted.
#[test]
fn skips_views_where_entity_is_only_prefix_only_ancestor() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let parent_path = EntityPath::from("/world/points");
    let child_path = EntityPath::from("/world/points/red");
    log_point(&mut test_context, &parent_path);
    log_point(&mut test_context, &child_path);

    let [view_a, view_b, view_prefix_only] =
        test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            [
                blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                    TestView::identifier(),
                )),
                blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                    TestView::identifier(),
                )),
                blueprint.add_view_at_root(ViewBlueprint::new(
                    TestView::identifier(),
                    RecommendedView {
                        origin: EntityPath::root(),
                        query_filter: EntityPathFilter::parse_forgiving("+ /world/points/red"),
                    },
                )),
            ]
        });

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &parent_path, false);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        assert_eq!(read_override(ctx, view_a, &parent_path), Some(false));
        assert_eq!(read_override(ctx, view_b, &parent_path), Some(false));
        assert_eq!(read_override(ctx, view_prefix_only, &parent_path), None);
    });
}

// ---------------------------------------------------------------------------

fn log_point(test_context: &mut TestContext, entity_path: &EntityPath) {
    test_context.log_entity(entity_path.clone(), |b| {
        b.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(0.0, 0.0)]),
        )
    });
}

fn three_wildcard_views(test_context: &mut TestContext) -> [ViewId; 3] {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        std::array::from_fn(|_| {
            blueprint
                .add_view_at_root(ViewBlueprint::new_with_root_wildcard(TestView::identifier()))
        })
    })
}

fn read_override(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    entity_path: &EntityPath,
) -> Option<bool> {
    let override_path = ViewContents::base_override_path_for_entity(view_id, entity_path);
    let component = EntityBehavior::descriptor_visible().component;
    ctx.store_context
        .blueprint
        .latest_at(ctx.blueprint_query, &override_path, [component])
        .component_mono::<Visible>(component)
        .map(|v| *v.0)
}

/// Force a rebuild of `ctx.query_results` so cached `DataResult` fields
/// reflect blueprint mutations made since the last setup. In a real viewer
/// this happens automatically each frame.
fn refresh_query_results(test_context: &mut TestContext) {
    test_context.setup_viewport_blueprint(|_, _| {});
}
