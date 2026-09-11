use egui_kittest::kittest::Queryable as _;
use re_integration_test::HarnessExt as _;
use re_integration_test::ViewerHarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_log_types::EntityPath;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_view_entity_picker_scrolls_long_entity_lists() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(800.0, 600.0)),
        max_steps: Some(100),
        ..Default::default()
    });
    harness.init_recording();
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(false);

    for index in 0..100 {
        harness.log_entity(format!("points/entity_{index:03}"), |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Points2D::new([[index as f32, index as f32]]),
            )
        });
    }

    harness.clear_current_blueprint();
    let view_id = harness.setup_viewport_blueprint(|_ctx, blueprint| {
        let mut view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView2D::identifier());
        view.display_name = Some("2D view".into());
        let view_id = view.id;
        blueprint.add_view_at_root(view);
        view_id
    });

    harness.blueprint_tree().click_label("2D view");
    harness
        .selection_panel()
        .click_label("Modify the entity query using the editor");

    let first_y_before = harness
        .root_section()
        .get_label("entity_000")
        .rect()
        .center()
        .y;
    harness.root_section().hover_label("entity_000");
    for _ in 0..4 {
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Page,
            delta: egui::vec2(0.0, -1.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }

    let first_y_after = harness
        .root_section()
        .get_label("entity_000")
        .rect()
        .center()
        .y;
    assert!(
        first_y_after < first_y_before,
        "scrolling the entity picker should move its entity list"
    );
    let last_entity_rect = harness.root_section().get_label("entity_099").rect();
    let modal_title_rect = harness
        .root_section()
        .get_label("Add/remove Entities")
        .rect();
    let window_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    assert!(
        window_rect.contains_rect(last_entity_rect)
            && last_entity_rect.top() > modal_title_rect.bottom(),
        "the last entity should be fully visible below the modal title after scrolling"
    );

    let exclude_button = harness
        .query_all_by_label("Exclude entity")
        .min_by(|left, right| {
            (left.rect().center().y - last_entity_rect.center().y)
                .abs()
                .total_cmp(&(right.rect().center().y - last_entity_rect.center().y).abs())
        })
        .expect("entity_099 should have an exclude action");
    exclude_button.click();
    harness.run();
    assert!(
        harness.run_with_viewer_context(move |ctx| {
            ctx.lookup_query_result(view_id)
                .result_for_entity(&EntityPath::from("points/entity_099"))
                .is_none()
        }),
        "excluding entity_099 should remove it from the view"
    );

    let remove_rule_button = harness
        .query_all_by_label("Remove this rule")
        .min_by(|left, right| {
            (left.rect().center().y - last_entity_rect.center().y)
                .abs()
                .total_cmp(&(right.rect().center().y - last_entity_rect.center().y).abs())
        })
        .expect("entity_099 should have a remove-rule action");
    remove_rule_button.click();
    harness.run();
    assert!(
        harness.run_with_viewer_context(move |ctx| {
            ctx.lookup_query_result(view_id)
                .result_for_entity(&EntityPath::from("points/entity_099"))
                .is_some()
        }),
        "removing the exclusion should add entity_099 back to the view"
    );
}
