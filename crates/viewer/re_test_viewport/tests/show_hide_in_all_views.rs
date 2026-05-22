//! Tests for [`re_viewer_context::set_entity_visibility_in_all_views`].

use re_chunk::TimePoint;
use re_entity_db::EntityPath;
use re_log_types::EntityPathFilter;
use re_log_types::example_components::{MyPoint, MyPoints};
use re_sdk_types::blueprint::archetypes::EntityBehavior;
use re_sdk_types::components::Visible;
use re_test_context::TestContext;
use re_test_viewport::{TestContextExt as _, TestView};
use re_viewer_context::{
    RecommendedView, ViewClass as _, ViewId, ViewerContext, set_entity_visibility_in_all_views,
};
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

/// 3 views all containing `/world/points`; calling the helper with `false`
/// should write a `visible=Some(false)` override at each view's per-entity
/// override base path.
#[test]
fn hide_in_all_views_writes_visible_override_to_each_containing_view() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let entity_path = EntityPath::from("/world/points");
    log_point(&mut test_context, &entity_path, (0.0, 0.0));

    let view_ids = three_wildcard_views(&mut test_context);

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &entity_path, false);
    });

    // Flush deferred blueprint writes so the next frame can read them back.
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            let override_path = ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                Some(false),
                "view {view_id:?} at {override_path}",
            );
        }
    });
}

/// Pre-state: `visible=false` overrides on all 3 views.
/// Calling the helper with `true` should clear the override on each peer
/// (because the parent default visibility is `true`), not write `Some(true)`.
/// This pins the per-peer smart-clear behavior that comes from reusing
/// `DataResult::save_visible`.
#[test]
fn show_in_all_views_clears_overrides_when_matching_parent_default() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let entity_path = EntityPath::from("/world/points");
    log_point(&mut test_context, &entity_path, (0.0, 0.0));

    let view_ids = three_wildcard_views(&mut test_context);

    // Pre-state: hide on all 3 views via the helper itself.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &entity_path, false);
    });
    test_context.handle_system_commands(&egui::Context::default());

    // Refresh `query_results` so `data_result.is_visible()` reflects the new
    // overrides, mirroring what happens between real viewer frames.
    refresh_query_results(&mut test_context);

    // Sanity check: pre-state is as expected.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            let override_path = ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                Some(false),
                "pre-state: view {view_id:?}",
            );
        }
    });

    // Toggle back to default — should clear every override, not write `Some(true)`.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &entity_path, true);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            let override_path = ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                None,
                "view {view_id:?} after toggle-back",
            );
        }
    });
}

/// 3 views, but one of them uses a content filter that excludes `/world/points`.
/// The helper should only write overrides on the 2 views whose query result
/// contains the entity, and leave the third untouched.
#[test]
fn action_skips_views_that_do_not_contain_the_entity() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let entity_path = EntityPath::from("/world/points");
    log_point(&mut test_context, &entity_path, (0.0, 0.0));

    let [view_a, view_b, view_excluded] =
        test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            [
                blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                    TestView::identifier(),
                )),
                blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                    TestView::identifier(),
                )),
                blueprint.add_view_at_root(filtered_view("+ /elsewhere/**")),
            ]
        });

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &entity_path, false);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in [view_a, view_b] {
            let override_path = ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                Some(false),
                "included view {view_id:?}",
            );
        }

        let excluded_path =
            ViewContents::base_override_path_for_entity(view_excluded, &entity_path);
        assert_eq!(
            read_visible_override(ctx, &excluded_path),
            None,
            "excluded view {view_excluded:?} at {excluded_path}",
        );
    });
}

/// Mixed pre-state: 2 of 3 views have explicit `visible=false` overrides on
/// the entity, the 3rd view is inheriting `visible=true` from its parent.
/// Calling the helper with `true` should clear the 2 overrides AND leave the
/// 3rd view untouched — writing a fresh `Some(true)` there would silently
/// sever parent-to-child visibility inheritance in that view.
#[test]
fn show_in_all_views_does_not_write_redundant_override_on_inheriting_view() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let entity_path = EntityPath::from("/world/points");
    log_point(&mut test_context, &entity_path, (0.0, 0.0));

    let view_ids = three_wildcard_views(&mut test_context);

    // Pre-state: hide on views 0 and 1 only; view 2 keeps the inherited default.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in [view_ids[0], view_ids[1]] {
            let query_result = ctx.query_results.get(&view_id).expect("query result");
            let data_result = query_result
                .tree
                .lookup_result_by_path(entity_path.hash())
                .expect("data result");
            data_result.save_visible(ctx, &query_result.tree, false);
        }
    });
    test_context.handle_system_commands(&egui::Context::default());
    refresh_query_results(&mut test_context);

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in [view_ids[0], view_ids[1]] {
            let override_path = ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                Some(false),
                "pre-state: view {view_id:?}",
            );
        }
        let inheriting_view = view_ids[2];
        let inheriting_path =
            ViewContents::base_override_path_for_entity(inheriting_view, &entity_path);
        assert_eq!(
            read_visible_override(ctx, &inheriting_path),
            None,
            "pre-state: inheriting view {inheriting_view:?}",
        );
    });

    // Show everywhere.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &entity_path, true);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            let override_path = ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                None,
                "view {view_id:?} after toggle-back",
            );
        }
    });
}

/// 3 views, one of which uses a content filter that matches only
/// `/world/points/red`. `/world/points` then exists in that view's data-result
/// tree only as a synthesized prefix-only ancestor. The helper must skip it —
/// writing an override at `/world/points` in that view would cascade to
/// `/world/points/red`, hiding an entity the user never targeted.
#[test]
fn action_skips_views_where_entity_is_only_a_prefix_only_ancestor() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();
    let parent_path = EntityPath::from("/world/points");
    let child_path = EntityPath::from("/world/points/red");
    log_point(&mut test_context, &parent_path, (0.0, 0.0));
    log_point(&mut test_context, &child_path, (1.0, 1.0));

    let [view_a, view_b, view_prefix_only] =
        test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            [
                blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                    TestView::identifier(),
                )),
                blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                    TestView::identifier(),
                )),
                blueprint.add_view_at_root(filtered_view("+ /world/points/red")),
            ]
        });

    // Sanity check: in the prefix-only view, /world/points is a tree-prefix
    // ancestor (the view's filter matches only the child), while
    // /world/points/red is a real DataResult.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let tree = &ctx
            .query_results
            .get(&view_prefix_only)
            .expect("query result")
            .tree;
        let parent_dr = tree
            .lookup_result_by_path(parent_path.hash())
            .expect("prefix-only DataResult for /world/points");
        assert!(
            parent_dr.tree_prefix_only,
            "/world/points should be a prefix-only ancestor in the filtered view",
        );
        let child_dr = tree
            .lookup_result_by_path(child_path.hash())
            .expect("DataResult for /world/points/red");
        assert!(
            !child_dr.tree_prefix_only,
            "/world/points/red should be a real DataResult in the filtered view",
        );
    });

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        set_entity_visibility_in_all_views(ctx, &parent_path, false);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in [view_a, view_b] {
            let override_path = ViewContents::base_override_path_for_entity(view_id, &parent_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                Some(false),
                "view {view_id:?} on /world/points",
            );
        }

        let prefix_only_override =
            ViewContents::base_override_path_for_entity(view_prefix_only, &parent_path);
        assert_eq!(
            read_visible_override(ctx, &prefix_only_override),
            None,
            "prefix-only view {view_prefix_only:?} at /world/points",
        );
    });
}

// ---------------------------------------------------------------------------

fn log_point(test_context: &mut TestContext, entity_path: &EntityPath, point: (f32, f32)) {
    test_context.log_entity(entity_path.clone(), |b| {
        b.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(point.0, point.1)]),
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

fn filtered_view(filter: &str) -> ViewBlueprint {
    ViewBlueprint::new(
        TestView::identifier(),
        RecommendedView {
            origin: EntityPath::root(),
            query_filter: EntityPathFilter::parse_forgiving(filter),
        },
    )
}

fn read_visible_override(ctx: &ViewerContext<'_>, override_path: &EntityPath) -> Option<bool> {
    let component = EntityBehavior::descriptor_visible().component;
    ctx.store_context
        .blueprint
        .latest_at(ctx.blueprint_query, override_path, [component])
        .component_mono::<Visible>(component)
        .map(|v| *v.0)
}

/// Force a rebuild of `ctx.query_results` so cached `DataResult` fields
/// (visible, interactive, …) reflect any blueprint mutations made since
/// the last setup. In a real viewer this happens automatically each frame.
fn refresh_query_results(test_context: &mut TestContext) {
    test_context.setup_viewport_blueprint(|_, _| {});
}
