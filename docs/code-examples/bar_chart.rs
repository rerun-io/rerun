//! Create and log a bar chart

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_bar_chart").memory()?;

    rec.log(
        "bar_chart",
        &rerun::BarChart::new(vec![8_i64, 4, 0, 9, 1, 4, 1, 6, 9, 0]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
