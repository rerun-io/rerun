//! Log a batch of 3D arrows.

use rerun::{
    archetypes::{Arrows3D, Clear},
    components::Color,
    external::glam,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_clear_simple").memory()?;

    #[rustfmt::skip]
    let (vectors, origins, colors) = (
        [glam::Vec3::X,    glam::Vec3::NEG_Y, glam::Vec3::NEG_X, glam::Vec3::Y],
        [(-0.5, 0.5, 0.0), (0.5, 0.5, 0.0),   (0.5, -0.5, 0.0),  (-0.5, -0.5, 0.0)],
        [(200, 0, 0),      (0, 200, 0),       (0, 0, 200),       (200, 0, 200)],
    );

    // Log a handful of arrows.
    for (i, ((vector, origin), color)) in vectors.into_iter().zip(origins).zip(colors).enumerate() {
        rec.log(
            format!("arrows/{i}"),
            &Arrows3D::from_vectors([vector])
                .with_origins([origin])
                .with_colors([Color::from_rgb(color.0, color.1, color.2)]),
        )?;
    }

    // Now clear them, one by one on each tick.
    for i in 0..vectors.len() {
        rec.log(format!("arrows/{i}"), &Clear::flat())?;
    }

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
