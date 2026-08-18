use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::Harness;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::StateTimelineView;
use re_viewer_context::{Item, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

// Same layout constants the view uses; see `view_class.rs`.
const LANE_BAND_HEIGHT: f32 = 22.0;
const LANE_LABEL_HEIGHT: f32 = 14.0;
const TIME_AXIS_HEIGHT: f32 = 20.0;
const TOP_MARGIN: f32 = 4.0;

const VIEW_SIZE: egui::Vec2 = egui::vec2(500.0, 250.0);

const ENTITY: &str = "state/robot";

fn log_data(test_context: &mut TestContext, timeline: Timeline) {
    for (tick, state) in [(0, "Idle"), (10, "Moving"), (20, "Idle"), (30, "Charging")] {
        test_context.log_entity(ENTITY, |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &re_sdk_types::archetypes::StateChange::single(state),
            )
        });
    }
}

fn click_at(harness: &mut Harness<'_>, pos: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    // The selection is applied through a system command, which lands on the next frame.
    harness.step();
}

fn selected_item(test_context: &TestContext) -> Option<Item> {
    test_context
        .selection_state
        .lock()
        .selected_items()
        .single_item()
        .cloned()
}

#[test]
fn test_clicking_a_phase_selects_the_entity() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::new_sequence("tick");
    log_data(&mut test_context, timeline);
    test_context.set_active_timeline(*timeline.name());

    let view_id: ViewId = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            StateTimelineView::identifier(),
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui(VIEW_SIZE)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();

    // Middle of the single lane's band, horizontally in the middle of the phases.
    let lane_y = TIME_AXIS_HEIGHT + TOP_MARGIN + LANE_LABEL_HEIGHT + LANE_BAND_HEIGHT / 2.0;
    click_at(&mut harness, egui::pos2(VIEW_SIZE.x / 2.0, lane_y));

    match selected_item(&test_context) {
        Some(Item::DataResult(address)) => {
            assert_eq!(address.view_id, view_id);
            assert_eq!(address.instance_path.entity_path, EntityPath::from(ENTITY));
        }
        other => panic!("expected the clicked entity to be selected, got {other:?}"),
    }

    // Clicking well below all lanes still selects the view itself.
    click_at(
        &mut harness,
        egui::pos2(VIEW_SIZE.x / 2.0, VIEW_SIZE.y - 10.0),
    );
    assert_eq!(
        selected_item(&test_context),
        Some(Item::View(view_id)),
        "clicking empty space should select the view"
    );
}
