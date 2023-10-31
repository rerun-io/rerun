//! Log and then clear data.

use rerun::external::glam;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_clear_simple").spawn()?;

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
            &rerun::Arrows3D::from_vectors([vector])
                .with_origins([origin])
                .with_colors([rerun::Color::from_rgb(color.0, color.1, color.2)]),
        )?;
    }

    // Now clear them, one by one on each tick.
    for i in 0..vectors.len() {
        rec.log(format!("arrows/{i}"), &rerun::Clear::flat())?;
    }

    Ok(())
}
