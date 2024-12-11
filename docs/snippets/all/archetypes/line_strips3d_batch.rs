//! Log a batch of 2D line strips.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip3d_batch").spawn()?;

    let strip1 = [[0., 0., 2.], [1., 0., 2.], [1., 1., 2.], [0., 1., 2.]];
    let strip2 = [
        [0., 0., 0.],
        [0., 0., 1.],
        [1., 0., 0.],
        [1., 0., 1.],
        [1., 1., 0.],
        [1., 1., 1.],
        [0., 1., 0.],
        [0., 1., 1.],
    ];
    rec.log(
        "strips",
        &rerun::LineStrips3D::new([strip1.to_vec(), strip2.to_vec()])
            .with_colors([0xFF0000FF, 0x00FF00FF])
            .with_radii([0.025, 0.005])
            .with_labels(["one strip here", "and one strip there"]),
    )?;

    Ok(())
}
