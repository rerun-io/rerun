use re_chunk_store::RowId;
use re_log_types::{TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_context::external::egui_kittest::kittest::Queryable as _;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::StateTimelineView;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

/// 2025-04-01 12:00:00 UTC, in nanoseconds since epoch.
const BASE_NS: i64 = 1_743_508_800_000_000_000;

/// 5 seconds.
const STEP_NS: i64 = 5_000_000_000;

const LONG_LABEL: &str = "Recovering from an unexpected joint torque limit";

/// Vertical center of the first lane band: time axis, top margin, lane label, then half a band.
const LANE_Y: f32 = 20.0 + 4.0 + 14.0 + 11.0;

/// Log three phases on a timestamp timeline and put them in a state timeline view.
///
/// The short state comes first, so hovering it and *then* the long one reproduces the
/// wrapping regression.
fn setup(test_context: &mut TestContext) -> ViewId {
    let timeline = Timeline::new_timestamp("timestamp");

    let state_data: [(i64, &str); 3] = [(0, "Idle"), (1, LONG_LABEL), (2, "Idle")];
    for (step, state) in &state_data {
        test_context.log_entity("state/robot_mode", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, BASE_NS + *step * STEP_NS)]),
                &re_sdk_types::archetypes::StateChange::single(*state),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            StateTimelineView::identifier(),
        ))
    })
}

/// The tooltip of a hovered phase shows the state label and the phase boundaries.
///
/// Two things this pins down (RR-4608):
/// * The boundaries are shown as a plain time-of-day, without the date.
/// * A long label doesn't get wrapped to the width the tooltip happened to have when it was
///   first shown — the tooltip is re-widened on every phase.
#[test]
fn test_state_timeline_tooltip() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let view_id = setup(&mut test_context);

    let size = egui::vec2(800.0, 300.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    // Let the view auto-fit and settle.
    harness.run();

    // The view auto-fits the data plus a trailing overhang, so the three phases sit in the
    // left ~3/4 of the view: `Idle`, then the long label, then `Idle` again.
    // The tooltip fades in, which keeps the ui repainting, so step a fixed number of frames
    // rather than `run()` (which bails out on continuous repaints).
    harness.hover_at(egui::pos2(size.x * 0.1, LANE_Y));
    harness.run_steps(20);
    snapshot_results.add(harness.try_snapshot("state_timeline_tooltip_short_label"));

    // Move onto the long phase without leaving the view: the tooltip is the same egui area,
    // which must not keep the width it had over the short phase.
    harness.hover_at(egui::pos2(size.x * 0.6, LANE_Y));
    harness.run_steps(20);
    snapshot_results.add(harness.try_snapshot("state_timeline_tooltip_long_label"));

    // The date is dropped even though the test's app options ask for `ShowDate`, and even
    // though the recording is not from today. The prefix and the time are separate labels, so
    // that the times of the two boundaries line up in a column.
    assert!(
        harness.query_by_label_contains("12:00:05Z").is_some(),
        "the tooltip should show the phase start as a plain time-of-day, without the date"
    );
    assert!(
        harness.query_by_label_contains("2025-04-01").is_none(),
        "no tooltip label should carry a date"
    );
}

/// In a viewport narrower than the regular tooltip width, a long label has to wrap rather
/// than be painted outside the viewport.
#[test]
fn test_state_timeline_tooltip_narrow_viewport() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let view_id = setup(&mut test_context);

    // Narrower than `tooltip_width` (600 points).
    let size = egui::vec2(300.0, 200.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    harness.hover_at(egui::pos2(size.x * 0.6, LANE_Y));
    harness.run_steps(20);
    snapshot_results.add(harness.try_snapshot("state_timeline_tooltip_narrow_viewport"));
}

/// The last phase has no end time, so its tooltip shows a dash instead of naming one.
///
/// This gets its own harness: hovering a second phase in the same one would put the pointer
/// over the tooltip left behind by the first, which then swallows the hover.
#[test]
fn test_state_timeline_tooltip_ongoing() {
    let mut snapshot_results = SnapshotResults::new();
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();

    let view_id = setup(&mut test_context);

    let size = egui::vec2(800.0, 300.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    harness.run();

    // The last phase sits at the right end, past the trailing overhang.
    harness.hover_at(egui::pos2(size.x * 0.95, LANE_Y));
    harness.run_steps(20);
    // The snapshot is the assertion here: the `End:` row carries a dash.
    snapshot_results.add(harness.try_snapshot("state_timeline_tooltip_ongoing"));
}
