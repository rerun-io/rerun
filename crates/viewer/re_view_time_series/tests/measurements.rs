use re_log_types::{TimePoint, Timeline};
use re_sdk_types::archetypes::Measurements;
use re_sdk_types::components::Color;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_time_series::TimeSeriesView;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TimeSeriesView::identifier(),
        ))
    })
}

/// Two measurement series, each with its own variance drawn as an error band.
///
/// Series 0 has `variance == 0` for one window, so the snapshot covers both the band and the
/// no-band path.
#[test]
fn test_measurements_band_rendering() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = Timeline::log_tick();

    test_context.log_entity("pressure", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::default(),
            &Measurements::update_fields()
                .with_widths([3.0, 3.0])
                .with_colors([
                    Color::from_rgb(120, 180, 255),
                    Color::from_rgb(255, 160, 120),
                ])
                .with_names(["sensor_a", "sensor_b"]),
        )
    });

    for step in 0..32_i64 {
        let timepoint = TimePoint::from([(timeline, step)]);
        let value_a = (step as f64 / 5.0).sin();
        let value_b = (step as f64 / 6.0).cos() * 0.6;
        let variance_a = if (10..=15).contains(&step) {
            0.0
        } else {
            0.02 + 0.015 * (step as f64 / 7.0).cos().abs()
        };
        let variance_b = 0.015 + 0.01 * (step as f64 / 9.0).sin().abs();
        test_context.log_entity("pressure", |builder| {
            builder.with_archetype_auto_row(
                timepoint,
                &Measurements::new([value_a, value_b])
                    .with_variances([variance_a, variance_b])
                    .with_units(["Pa", "Pa"]),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "measurements_band_rendering",
        egui::vec2(300.0, 300.0),
        None,
    ));
}

/// The band has to survive aggregation: with far more points than pixels, the min/max aggregators
/// collapse whole windows into single points and the band must still envelope the line.
#[test]
fn test_measurements_band_aggregation() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = Timeline::log_tick();

    test_context.log_entity("pressure", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::default(),
            &Measurements::update_fields()
                .with_colors([Color::from_rgb(120, 180, 255)])
                .with_aggregation_policy(re_sdk_types::components::AggregationPolicy::MinMax),
        )
    });

    // Far more points than the 300px-wide plot can show, so aggregation kicks in.
    for step in 0..4096_i64 {
        let value = (step as f64 / 50.0).sin() + 0.2 * (step as f64).sin();
        // Variance swings inside each aggregation window, so the band varies point to point.
        let variance = 0.002 + 0.02 * (step as f64 / 3.0).sin().abs();
        test_context.log_entity("pressure", |builder| {
            builder.with_archetype_auto_row(
                TimePoint::from([(timeline, step)]),
                &Measurements::new([value]).with_variances([variance]),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context);
    let mut snapshot_results = SnapshotResults::new();
    // Thousands of aggregated quads leave far more soft band edges than the ui defaults tolerate.
    let size = egui::vec2(300.0, 300.0);
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "measurements_band_aggregation",
        size,
        Some(re_ui::testing::default_snapshot_options_for_3d(size)),
    ));
}

/// The band has to follow the staircase, not cut across it: `StepAfter` holds each value until the
/// next sample, so the band edges have to be stepped the same way the line is.
#[test]
fn test_measurements_stepped_band() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = Timeline::log_tick();

    test_context.log_entity("pressure", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::default(),
            &Measurements::update_fields()
                .with_colors([Color::from_rgb(120, 180, 255)])
                .with_widths([2.0])
                .with_interpolation_mode(re_sdk_types::components::InterpolationMode::StepAfter),
        )
    });

    // Few, widely spaced samples so the steps are several pixels wide.
    for step in 0..8_i64 {
        test_context.log_entity("pressure", |builder| {
            builder.with_archetype_auto_row(
                TimePoint::from([(timeline, step)]),
                &Measurements::new([(step as f64 / 2.0).sin()])
                    .with_variances([0.02 + 0.03 * (step as f64 / 3.0).cos().abs()]),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "measurements_stepped_band",
        egui::vec2(300.0, 300.0),
        None,
    ));
}

/// Timestamps with fewer scalars than series turn the missing ones into `Clear` points, which carry
/// `value == 0`. Those must not be banded, or a phantom band appears around y = 0.
#[test]
fn test_measurements_cleared_series_has_no_band() {
    let mut test_context = TestContext::new_with_view_class::<TimeSeriesView>();
    let timeline = Timeline::log_tick();

    test_context.log_entity("pressure", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::default(),
            &Measurements::update_fields()
                .with_widths([2.0, 2.0])
                .with_colors([
                    Color::from_rgb(120, 180, 255),
                    Color::from_rgb(255, 160, 120),
                ]),
        )
    });

    // A single variance value gets clamped across both series, so the second series' `Clear` points
    // pick up a non-zero band offset.
    for step in 0..32_i64 {
        let value_a = 2.0 + (step as f64 / 5.0).sin();
        test_context.log_entity("pressure", |builder| {
            builder.with_archetype_auto_row(
                TimePoint::from([(timeline, step)]),
                &if (8..24).contains(&step) {
                    // Series 1 is missing here.
                    Measurements::new([value_a])
                } else {
                    Measurements::new([value_a, 4.0 + (step as f64 / 6.0).cos()])
                }
                .with_variances([0.05]),
            )
        });
    }

    test_context.set_active_timeline(*timeline.name());

    let view_id = setup_blueprint(&mut test_context);
    let mut snapshot_results = SnapshotResults::new();
    snapshot_results.add(test_context.run_view_ui_and_save_snapshot(
        view_id,
        "measurements_cleared_series_has_no_band",
        egui::vec2(300.0, 300.0),
        None,
    ));
}
