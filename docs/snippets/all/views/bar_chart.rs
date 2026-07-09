//! Use a blueprint to show a bar chart.

use rerun::blueprint::{
    archetypes as blueprint_archetypes, BarChartView, Blueprint,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        BarChartView::new("Bar Chart")
            .with_origin("bar_chart")
            .with_background(
                blueprint_archetypes::PlotBackground::new()
                    .with_color([50, 0, 50, 255])
                    .with_show_grid(false),
            ),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_bar_chart")
        .with_blueprint(blueprint)
        .spawn()?;

    rec.log(
        "bar_chart",
        &rerun::BarChart::new([8_i64, 4, 0, 9, 1, 4, 1, 6, 9, 0].as_slice()),
    )?;

    Ok(())
}
