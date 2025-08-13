//! Create and log a bar chart

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_bar_chart").spawn()?;

    rec.log(
        "bar_chart",
        &rerun::BarChart::new([8_i64, 4, 0, 9, 1, 4, 1, 6, 9, 0].as_slice()),
    )?;
    rec.log(
        "bar_chart_custom_abscissa",
        &rerun::BarChart::new([8_i64, 4, 0, 9, 1, 4].as_slice())
            .with_abscissa([0_i64, 1, 3, 4, 7, 11].as_slice()),
    )?;

    Ok(())
}
