//! Puts pressure on parallel visualizer execution, query caching and re-entrancy by scrubbing
//! the time cursor across many views that all read the same entities.

use re_integration_test::HarnessExt as _;
use re_integration_test::ViewerHarnessExt as _;
use re_sdk::Timeline;
use re_sdk::external::re_log_types::TimeReal;
use re_sdk_types::Archetype as _;
use re_view_text_log::TextView;
use re_view_time_series::TimeSeriesView;
use re_viewer::external::re_sdk_types::archetypes::{
    Arrows2D, Arrows3D, Boxes2D, Boxes3D, LineStrips2D, LineStrips3D, Points2D, Points3D, Scalars,
    TextLog,
};
use re_viewer::external::re_sdk_types::blueprint::archetypes::VisibleTimeRanges;
use re_viewer::external::re_sdk_types::blueprint::components::PlayState;
use re_viewer::external::re_sdk_types::datatypes::{
    TimeInt, TimeRange, TimeRangeBoundary, VisibleTimeRange,
};
use re_viewer::external::re_view_spatial::{SpatialView2D, SpatialView3D};
use re_viewer::external::re_viewer_context::{
    BlueprintContext as _, RecommendedView, TimeControlCommand, ViewClass as _, ViewId,
    ViewerContext,
};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

const NUM_FRAMES: i64 = 50;
const NUM_INSTANCES: usize = 32;

/// The order in which we jump around the timeline: alternating between both ends, closing in on
/// the middle. Every seek invalidates the caches of every view.
///
/// All of these have to stay within `0..NUM_FRAMES`, otherwise the cursor gets clamped and the
/// assertions in the seek loop no longer hold.
const SEEKS: [i64; 10] = [0, 49, 1, 48, 10, 39, 20, 29, 24, 25];

fn set_visible_time_range(
    ctx: &ViewerContext<'_>,
    view_id: ViewId,
    start: TimeRangeBoundary,
    end: TimeRangeBoundary,
) {
    let storage_engine = ctx.store_context.blueprint.storage_engine();
    let blueprint_tree = storage_engine.store().entity_tree();
    let property_path = re_viewport_blueprint::entity_path_for_view_property(
        view_id,
        blueprint_tree,
        VisibleTimeRanges::name(),
    );
    ctx.save_blueprint_archetype(
        property_path,
        &VisibleTimeRanges::new([VisibleTimeRange {
            timeline: "frame_nr".into(),
            range: TimeRange { start, end },
        }]),
    );
}

fn cursor_relative(offset: i64) -> TimeRangeBoundary {
    TimeRangeBoundary::CursorRelative(TimeInt(offset))
}

#[tokio::test(flavor = "multi_thread")]
async fn rapid_time_scrubbing_across_many_views() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1024.0, 768.0)),
        max_steps: Some(4),
        step_dt: Some(1.0 / 60.0),
        // The spatial views render with `re_renderer`, which needs the 3D thresholds.
        snapshot_test_options: re_ui::testing::TestOptions::Rendering3D,
        ..Default::default()
    });
    harness.init_recording();
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    let timeline = Timeline::new_sequence("frame_nr");
    for frame in 0..NUM_FRAMES {
        let timepoint = [(timeline, frame)];
        let positions_2d = (0..NUM_INSTANCES)
            .map(|instance| {
                let phase = (frame as usize + instance) as f32 / 10.0;
                [phase.sin() * 10.0, phase.cos() * 10.0]
            })
            .collect::<Vec<_>>();
        let positions_3d = (0..NUM_INSTANCES)
            .map(|instance| {
                let phase = (frame as usize + instance) as f32 / 10.0;
                [phase.sin() * 10.0, phase.cos() * 10.0, instance as f32]
            })
            .collect::<Vec<_>>();
        harness.log_entity("plots/value", |builder| {
            builder
                .with_archetype_auto_row(timepoint, &Scalars::single((frame as f64 / 10.0).sin()))
        });
        harness.log_entity("logs/message", |builder| {
            builder.with_archetype_auto_row(timepoint, &TextLog::new(format!("Frame {frame}")))
        });
        harness.log_entity("2D/points", |builder| {
            builder.with_archetype_auto_row(timepoint, &Points2D::new(positions_2d.clone()))
        });
        harness.log_entity("2D/lines", |builder| {
            builder.with_archetype_auto_row(timepoint, &LineStrips2D::new([positions_2d.clone()]))
        });
        harness.log_entity("2D/arrows", |builder| {
            builder
                .with_archetype_auto_row(timepoint, &Arrows2D::from_vectors(positions_2d.clone()))
        });
        harness.log_entity("2D/boxes", |builder| {
            builder.with_archetype_auto_row(
                timepoint,
                &Boxes2D::from_half_sizes([[0.1, 0.1]; NUM_INSTANCES])
                    .with_centers(positions_2d.clone()),
            )
        });
        // Labels go through the per-frame text layout cache, so keep a few of them around.
        harness.log_entity("2D/labeled", |builder| {
            builder.with_archetype_auto_row(
                timepoint,
                &Points2D::new(positions_2d.iter().take(4).copied().collect::<Vec<_>>())
                    .with_labels((0..4).map(|i| format!("p{i}@{frame}"))),
            )
        });
        harness.log_entity("3D/points", |builder| {
            builder.with_archetype_auto_row(timepoint, &Points3D::new(positions_3d.clone()))
        });
        harness.log_entity("3D/lines", |builder| {
            builder.with_archetype_auto_row(timepoint, &LineStrips3D::new([positions_3d.clone()]))
        });
        harness.log_entity("3D/arrows", |builder| {
            builder
                .with_archetype_auto_row(timepoint, &Arrows3D::from_vectors(positions_3d.clone()))
        });
        harness.log_entity("3D/boxes", |builder| {
            builder.with_archetype_auto_row(
                timepoint,
                &Boxes3D::from_half_sizes([[0.1, 0.1, 0.1]; NUM_INSTANCES])
                    .with_centers(positions_3d.clone()),
            )
        });
        harness.log_entity("3D/labeled", |builder| {
            builder.with_archetype_auto_row(
                timepoint,
                &Points3D::new(positions_3d.iter().take(4).copied().collect::<Vec<_>>())
                    .with_labels((0..4).map(|i| format!("p{i}@{frame}"))),
            )
        });
    }

    // Pin the cursor before creating any views.
    harness.send_time_commands([
        TimeControlCommand::SetActiveTimeline(*timeline.name()),
        TimeControlCommand::Pause,
        TimeControlCommand::SetTime(TimeReal::from(0_i64)),
    ]);
    assert_eq!(
        harness.with_active_time_ctrl(|time_ctrl| time_ctrl.play_state()),
        PlayState::Paused
    );

    harness.setup_viewport_blueprint(|ctx, blueprint| {
        // Duplicated views: identical queries in several views at once, so a single seek fans
        // the same invalidated query out over multiple parallel visualizer runs.
        for _ in 0..2 {
            blueprint.add_view_at_root(ViewBlueprint::new(
                TimeSeriesView::identifier(),
                RecommendedView::new_subtree("/plots"),
            ));
            blueprint.add_view_at_root(ViewBlueprint::new(
                TextView::identifier(),
                RecommendedView::new_subtree("/logs"),
            ));
        }

        blueprint.add_view_at_root(ViewBlueprint::new(
            SpatialView2D::identifier(),
            RecommendedView::new_subtree("/2D"),
        ));
        blueprint.add_view_at_root(ViewBlueprint::new(
            SpatialView3D::identifier(),
            RecommendedView::new_subtree("/3D"),
        ));

        // Cursor-relative windows, both behind and ahead of the cursor.
        for (start, end) in [(-20, -10), (-10, 0), (0, 10), (10, 20)] {
            let view_id = blueprint.add_view_at_root(ViewBlueprint::new(
                TimeSeriesView::identifier(),
                RecommendedView::new_subtree("/plots"),
            ));
            set_visible_time_range(ctx, view_id, cursor_relative(start), cursor_relative(end));
        }

        for (class_identifier, origin) in [
            (SpatialView2D::identifier(), "/2D"),
            (SpatialView3D::identifier(), "/3D"),
        ] {
            let view_id = blueprint.add_view_at_root(ViewBlueprint::new(
                class_identifier,
                RecommendedView::new_subtree(origin),
            ));
            set_visible_time_range(
                ctx,
                view_id,
                cursor_relative(-(NUM_FRAMES - 10)),
                cursor_relative(0),
            );

            // Ranges that reach to the edges of the recording: every seek re-queries the
            // whole timeline for these.
            let view_id = blueprint.add_view_at_root(ViewBlueprint::new(
                class_identifier,
                RecommendedView::new_subtree(origin),
            ));
            set_visible_time_range(
                ctx,
                view_id,
                TimeRangeBoundary::Infinite,
                TimeRangeBoundary::Infinite,
            );
        }
    });

    for frame in SEEKS {
        harness.send_time_commands([TimeControlCommand::SetTime(TimeReal::from(frame))]);

        // A seek invalidates the caches, so these are the frames that render from a re-populated
        // cache. A fixed step count rather than `run`/`run_ok`, so that the snapshots below always
        // capture the same frame after a seek: `run_ok` stops as soon as the app stops asking for
        // repaints, which is a frame count we don't control.
        harness.run_steps(2);

        assert_eq!(
            harness.with_active_time_ctrl(|time_ctrl| time_ctrl.time()),
            Some(TimeReal::from(frame)),
            "cursor should stay where it was put, but drifted after seeking to {frame}"
        );

        // Also snapshot in the middle of the scrub, so that a view which only renders wrongly
        // while being scrubbed doesn't slip through by looking fine once we come to a rest.
        if frame == NUM_FRAMES - 1 {
            harness.snapshot_app("rapid_time_scrubbing_mid");
        }
    }

    harness.snapshot_app("rapid_time_scrubbing_final");
}
