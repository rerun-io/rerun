//! Log a batch of 2d line strips.

use rerun::{
    archetypes::LineStrips2D, components::Rect2D, datatypes::Vec4D, RecordingStreamBuilder,
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
    // TODO(#2786): Rect2D archetype
    rec.log_component_lists(
        "bounds",
        false,
        1,
        [&Rect2D::XCYCWH(Vec4D([3.0, 1.5, 8.0, 9.0]).into()) as _],
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
