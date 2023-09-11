//! Log a batch of 2d line strips.

use rerun::{
    archetypes::{Boxes2D, LineStrips2D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_line_strip2d").memory()?;

    let strip1 = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    #[rustfmt::skip]
    let strip2 = [[0., 3.], [1., 4.], [2., 2.], [3., 4.], [4., 2.], [5., 4.], [6., 3.]];
    rec.log(
        "strips",
        &LineStrips2D::new([strip1.to_vec(), strip2.to_vec()])
            .with_colors([0xFF0000FF, 0x00FF00FF])
            .with_radii([0.025, 0.005])
            .with_labels(["one strip here", "and one strip there"]),
    )?;

    // Log an extra rect to set the view bounds
    rec.log(
        "bounds",
        &Boxes2D::new([(4.0, 4.5)]).with_centers([(3.0, 1.5)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
