//! Log some very simple 2D boxes.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_box2d").spawn()?;

    rec.log(
        "simple",
        &rerun::Boxes2D::from_mins_and_sizes([(-1., -1.)], [(2., 2.)]),
    )?;

    Ok(())
}
