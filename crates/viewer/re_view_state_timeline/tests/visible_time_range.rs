//! The state timeline view honors the `VisibleTimeRanges` blueprint property, both as a view-wide
//! setting and as a per-entity override — the same way the time series view does.

use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint, Timeline, TimelineName};
use re_sdk_types::blueprint::archetypes::VisibleTimeRanges;
use re_sdk_types::{blueprint, encodings};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_state_timeline::StateTimelineView;
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

const MAX_TIME: i64 = 40;

/// Two lanes of state changes, so that a per-entity override can restrict one of them only.
fn log_data(test_context: &mut TestContext, timeline: Timeline) {
    for (tick, state) in [(0, "Idle"), (10, "Moving"), (20, "Idle"), (30, "Charging")] {
        test_context.log_entity("state/robot", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &re_sdk_types::archetypes::StateChange::single(state),
            )
        });
    }
    for (tick, state) in [(0, "On"), (25, "Off")] {
        test_context.log_entity("state/power", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::from([(timeline, tick)]),
                &re_sdk_types::archetypes::StateChange::single(state),
            )
        });
    }
    // Make sure the timeline extends past the last state change.
    test_context.log_entity("state/robot", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(timeline, MAX_TIME)]),
            &re_sdk_types::archetypes::StateChange::single("Charging"),
        )
    });
}

fn visible_time_range(
    timeline: &TimelineName,
    range: encodings::TimeRange,
) -> blueprint::components::VisibleTimeRange {
    blueprint::components::VisibleTimeRange(encodings::VisibleTimeRange {
        timeline: timeline.as_str().into(),
        range,
    })
}

fn setup_blueprint(
    test_context: &mut TestContext,
    timeline: &TimelineName,
    view_range: Option<encodings::TimeRange>,
    entity_range: Option<&(EntityPath, encodings::TimeRange)>,
) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(StateTimelineView::identifier());

        if let Some(view_range) = view_range {
            let property = re_viewport_blueprint::ViewProperty::from_archetype_for_view::<
                VisibleTimeRanges,
            >(ctx, view.id);
            property.save_blueprint_component(
                ctx,
                &VisibleTimeRanges::descriptor_ranges(),
                &visible_time_range(timeline, view_range),
            );
        }

        if let Some((entity_path, entity_range)) = entity_range {
            ctx.save_blueprint_archetype(
                ViewContents::base_override_path_for_entity(view.id, entity_path),
                &VisibleTimeRanges::new([visible_time_range(timeline, *entity_range)]),
            );
        }

        blueprint.add_view_at_root(view)
    })
}

fn absolute(start: i64, end: i64) -> encodings::TimeRange {
    encodings::TimeRange {
        start: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(start)),
        end: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(end)),
    }
}

/// A view-wide visible time range restricts every lane; nothing outside it is drawn, however far
/// the view is zoomed out.
#[test]
fn test_visible_time_range_for_view() {
    let timeline = Timeline::log_tick();

    let ranges = [
        (encodings::TimeRange::EVERYTHING, "everything"),
        (absolute(10, 25), "absolute"),
        (
            encodings::TimeRange {
                start: encodings::TimeRangeBoundary::CursorRelative(encodings::TimeInt(-5)),
                end: encodings::TimeRangeBoundary::CursorRelative(encodings::TimeInt(5)),
            },
            "around_cursor",
        ),
        (
            encodings::TimeRange {
                start: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(15)),
                end: encodings::TimeRangeBoundary::Infinite,
            },
            "absolute_until_end",
        ),
        (
            encodings::TimeRange {
                start: encodings::TimeRangeBoundary::Infinite,
                end: encodings::TimeRangeBoundary::Absolute(encodings::TimeInt(15)),
            },
            "start_until_absolute",
        ),
        // Entirely outside the data: the lanes go empty, but the view keeps its labels and its
        // axis rather than collapsing to the "no state data" placeholder.
        (absolute(100, 200), "disjoint"),
    ];

    let mut snapshot_results = SnapshotResults::new();
    for (range, name) in ranges {
        let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
        log_data(&mut test_context, timeline);
        test_context.set_active_timeline(*timeline.name());

        test_context.set_time(re_log_types::TimeInt::new_temporal(20));

        let view_id = setup_blueprint(&mut test_context, timeline.name(), Some(range), None);
        snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
            view_id,
            &format!("state_timeline_visible_time_range_{name}"),
            egui::vec2(500.0, 200.0),
            None,
        ));
    }
}

/// An override on a single entity restricts only that lane; the other one keeps the view-wide
/// default of showing everything.
#[test]
fn test_visible_time_range_for_entity() {
    let mut test_context = TestContext::new_with_view_class::<StateTimelineView>();
    let timeline = Timeline::log_tick();
    log_data(&mut test_context, timeline);
    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(
        &mut test_context,
        timeline.name(),
        None,
        Some(&(EntityPath::from("state/robot"), absolute(10, 25))),
    );

    test_context
        .run_view_ui_and_save_snapshot(
            view_id,
            "state_timeline_visible_time_range_per_entity",
            egui::vec2(500.0, 200.0),
            None,
        )
        .unwrap();
}
