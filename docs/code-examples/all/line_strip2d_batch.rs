//! Log a batch of 2d line strips.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip2d").spawn()?;

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

    // Log an extra rect to set the view bounds
    rec.log(
        "bounds",
        &rerun::Boxes2D::from_centers_and_sizes([(3.0, 1.5)], [(8.0, 9.0)]),
    )?;

    Ok(())
}
