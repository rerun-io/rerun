use re_log_types::TimePoint;
use re_sdk_types::blueprint::{
    archetypes::{PlotInteraction, PlotLegend},
    components::TooltipMode,
};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::{SnapshotResults, kittest::Queryable as _};
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{BlueprintContext as _, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

#[test]
fn test_tooltips() {
    let mut snapshot_results = SnapshotResults::new();

    for tooltip_mode in [TooltipMode::Nearest, TooltipMode::All] {
        test_tooltip_mode(tooltip_mode, &mut snapshot_results);
    }
}

fn test_tooltip_mode(tooltip_mode: TooltipMode, snapshot_results: &mut SnapshotResults) {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = test_context
        .active_timeline()
        .expect("test context should have an active timeline");

    for series_index in 0..12 {
        let entity_path = format!("plots/series_{series_index:02}");
        let is_measurement = matches!(series_index, 5 | 11);
        if is_measurement {
            test_context.log_entity(entity_path.as_str(), |builder| {
                builder.with_archetype_auto_row(
                    TimePoint::default(),
                    &re_sdk_types::archetypes::Measurements::update_fields()
                        .with_names([format!("series_{series_index:02}")]),
                )
            });
        } else {
            test_context.log_entity(entity_path.as_str(), |builder| {
                builder.with_archetype_auto_row(
                    TimePoint::default(),
                    &re_sdk_types::archetypes::SeriesLines::new()
                        .with_names([format!("series_{series_index:02}")]),
                )
            });
        }

        for time in 0..=10 {
            test_context.log_entity(entity_path.as_str(), |builder| {
                if is_measurement {
                    builder.with_archetype_auto_row(
                        [(timeline, time)],
                        &re_sdk_types::archetypes::Measurements::new([series_index as f64])
                            .with_units(["Pa"]),
                    )
                } else {
                    builder.with_archetype_auto_row(
                        [(timeline, time)],
                        &re_sdk_types::archetypes::Scalars::single(series_index as f64),
                    )
                }
            });
        }
    }

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(TimeSeriesView::identifier());
        let interaction = ViewProperty::from_archetype_for_view::<PlotInteraction>(ctx, view.id);
        ctx.save_blueprint_archetype(
            interaction.blueprint_store_path,
            &PlotInteraction::new().with_tooltip_mode(tooltip_mode),
        );

        // Disable legend.
        let legend = ViewProperty::from_archetype_for_view::<PlotLegend>(ctx, view.id);
        ctx.save_blueprint_archetype(
            legend.blueprint_store_path,
            &PlotLegend::new().with_visible(false),
        );
        blueprint.add_view_at_root(view)
    });

    let size = egui::vec2(500.0, 300.0);
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    harness.hover_at(egui::pos2(size.x * 0.43, size.y * 0.51));
    harness.run();

    let snapshot_name = match tooltip_mode {
        TooltipMode::Nearest => "tooltip_nearest",
        TooltipMode::All => "tooltip_all",
    };
    snapshot_results.add(harness.try_snapshot(snapshot_name));

    let omitted_series_label = harness.query_by_label_contains("… and 2 more");
    match tooltip_mode {
        TooltipMode::Nearest => {
            assert!(
                omitted_series_label.is_none(),
                "the nearest tooltip should only show the hovered series"
            );
        }
        TooltipMode::All => assert!(
            omitted_series_label.is_some(),
            "the shared tooltip should cap its visible rows and report omitted series"
        ),
    }
}
