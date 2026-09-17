//! Tests for [`re_view::time_axis_view_range`].

use re_log_types::EntityPath;
use re_sdk_types::blueprint::{archetypes::TimeAxis, components::LinkAxis};
use re_sdk_types::encodings::{TimeRange, TimeRangeBoundary};
use re_test_context::TestContext;
use re_test_viewport::{TestContextExt as _, TestView};
use re_viewer_context::{GLOBAL_VIEW_ID, ViewClass as _, ViewClassExt as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Independent and linked views read their respective blueprint ranges.
/// Range updates and resets go to that same view, leaving the other view's stored range alone.
/// A view with no stored range reads the one `TestView` provides as fallback.
#[test]
fn time_axis_view_range_follows_link() {
    for link in [LinkAxis::Independent, LinkAxis::LinkToGlobal] {
        let mut test_context = TestContext::new_with_view_class::<TestView>();

        let local_range = TimeRange::from_cursor_plus_minus(100);
        let global_range = TimeRange::from_cursor_plus_minus(200);
        let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
            let view = ViewBlueprint::new_with_root_wildcard(TestView::identifier());
            let property = ViewProperty::from_archetype_for_view::<TimeAxis>(ctx, view.id);
            property.save_blueprint_component(ctx, &TimeAxis::descriptor_link(), &link);
            for (id, range) in [(view.id, local_range), (GLOBAL_VIEW_ID, global_range)] {
                ViewProperty::from_archetype_for_view::<TimeAxis>(ctx, id)
                    .save_blueprint_component(
                        ctx,
                        &TimeAxis::descriptor_view_range(),
                        &re_sdk_types::blueprint::components::TimeRange(range),
                    );
            }
            blueprint.add_view_at_root(view)
        });

        let read_range = |test_context: &TestContext| {
            test_context.run_once_in_egui_central_panel(|ctx, _ui| {
                let state = TestView.new_state();
                let origin = EntityPath::root();
                let view_ctx = TestView.view_context(ctx, view_id, state.as_ref(), &origin);
                re_view::time_axis_view_range(&view_ctx).unwrap().1.0
            })
        };

        // The range stored in the blueprint, without falling back.
        let stored_range = |test_context: &TestContext, view_id: ViewId| {
            test_context.with_blueprint_ctx(|ctx, _store_hub| {
                ViewProperty::from_archetype_for_view::<TimeAxis>(&ctx, view_id)
                    .component_or_empty::<re_sdk_types::blueprint::components::TimeRange>(
                        TimeAxis::descriptor_view_range().component,
                    )
                    .unwrap()
                    .map(|range| range.0)
            })
        };

        // The link decides which view holds the range.
        let (range_view_id, expected_range, other_view_id, other_range) = match link {
            LinkAxis::Independent => (view_id, local_range, GLOBAL_VIEW_ID, global_range),
            LinkAxis::LinkToGlobal => (GLOBAL_VIEW_ID, global_range, view_id, local_range),
        };
        let range_property = |ctx: &re_test_context::TestBlueprintCtx<'_>| {
            ViewProperty::from_archetype_for_view::<TimeAxis>(ctx, range_view_id)
        };

        assert_eq!(read_range(&test_context), expected_range);

        let new_range = TimeRange {
            start: TimeRangeBoundary::Absolute(300.into()),
            end: TimeRangeBoundary::Absolute(500.into()),
        };
        test_context.with_blueprint_ctx(|ctx, _store_hub| {
            range_property(&ctx).save_blueprint_component(
                &ctx,
                &TimeAxis::descriptor_view_range(),
                &re_sdk_types::blueprint::components::TimeRange(new_range),
            );
        });
        test_context.handle_system_commands(&egui::Context::default());

        assert_eq!(read_range(&test_context), new_range);
        assert_eq!(
            stored_range(&test_context, other_view_id),
            Some(other_range)
        );

        test_context.with_blueprint_ctx(|ctx, _store_hub| {
            range_property(&ctx).reset_blueprint_component(&ctx, TimeAxis::descriptor_view_range());
        });
        test_context.handle_system_commands(&egui::Context::default());

        assert_eq!(stored_range(&test_context, range_view_id), None);
        assert_eq!(
            stored_range(&test_context, other_view_id),
            Some(other_range)
        );
        assert_eq!(read_range(&test_context), TimeRange::EVERYTHING);
    }
}
