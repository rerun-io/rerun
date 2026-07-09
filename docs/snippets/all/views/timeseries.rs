//! Use a blueprint to customize a TimeSeriesView.

use rerun::blueprint::{
    Blueprint, TimeSeriesView, Vertical, archetypes as blueprint_archetypes,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(Vertical::new([
        TimeSeriesView::new("Y axis and time ranges")
            .with_origin("/trig")
            .with_axis_y(
                blueprint_archetypes::ScalarAxis::new()
                    .with_range([-1.0, 1.0])
                    .with_zoom_lock(true),
            )
            .with_plot_legend(
                blueprint_archetypes::PlotLegend::new().with_visible(false),
            )
            .with_time_ranges(blueprint_archetypes::VisibleTimeRanges::new([
                rerun::datatypes::VisibleTimeRange {
                    timeline: "timeline0".into(),
                    range: rerun::datatypes::TimeRange {
                        start:
                            rerun::datatypes::TimeRangeBoundary::CursorRelative(
                                (-100).into(),
                            ),
                        end: rerun::datatypes::TimeRangeBoundary::AT_CURSOR,
                    },
                },
                rerun::datatypes::VisibleTimeRange {
                    timeline: "timeline1".into(),
                    range: rerun::datatypes::TimeRange {
                        start: rerun::datatypes::TimeRangeBoundary::Absolute(
                            300_000_000_000_i64.into(),
                        ),
                        end: rerun::datatypes::TimeRangeBoundary::Infinite,
                    },
                },
            ]))
            .into(),
        TimeSeriesView::new("X axis and background")
            .with_origin("/trig")
            .with_axis_x(
                blueprint_archetypes::TimeAxis::new()
                    .with_view_range(
                        rerun::datatypes::TimeRange::from_cursor_plus_minus(
                            100,
                        ),
                    )
                    .with_zoom_lock(true),
            )
            .with_plot_legend(
                blueprint_archetypes::PlotLegend::new().with_visible(true),
            )
            .with_background(
                blueprint_archetypes::PlotBackground::new()
                    .with_color([128, 128, 128])
                    .with_show_grid(false),
            )
            .into(),
    ]));

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_timeseries")
        .with_blueprint(blueprint)
        .spawn()?;

    rec.log_static(
        "trig/sin",
        &rerun::SeriesLines::new()
            .with_colors([[255, 0, 0]])
            .with_names(["sin(0.01t)"]),
    )?;
    rec.log_static(
        "trig/cos",
        &rerun::SeriesLines::new()
            .with_colors([[0, 255, 0]])
            .with_names(["cos(0.01t)"]),
    )?;
    rec.log_static(
        "trig/cos_scaled",
        &rerun::SeriesLines::new()
            .with_colors([[0, 0, 255]])
            .with_names(["cos(0.01t) scaled"]),
    )?;

    for t in 0..((std::f64::consts::PI * 4.0 * 100.0) as i64) {
        rec.set_time_sequence("timeline0", t);
        rec.set_duration_secs("timeline1", t as f64);
        rec.log(
            "trig/sin",
            &rerun::Scalars::single((t as f64 / 100.0).sin()),
        )?;
        rec.log(
            "trig/cos",
            &rerun::Scalars::single((t as f64 / 100.0).cos()),
        )?;
        rec.log(
            "trig/cos_scaled",
            &rerun::Scalars::single((t as f64 / 100.0).cos() * 2.0),
        )?;
    }

    Ok(())
}
