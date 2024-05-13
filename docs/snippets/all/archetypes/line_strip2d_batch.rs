//! Log a batch of 2D line strips.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip2d_batch").spawn()?;

    let strip1 = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    #[rustfmt::skip]
    let strip2 = [[0., 3.], [1., 4.], [2., 2.], [3., 4.], [4., 2.], [5., 4.], [6., 3.]];
    rec.log(
        "strips",
        &rerun::LineStrips2D::new([strip1.to_vec(), strip2.to_vec()])
            .with_colors([0xFF0000FF, 0x00FF00FF])
            .with_radii([0.025, 0.005])
            .with_labels(["one strip here", "and one strip there"]),
    )?;

    // TODO(#5521): log VisualBounds

    Ok(())
}
