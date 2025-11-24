//! Update specific properties of a transform over time.

use rerun::AsComponents;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_transform3d_partial_updates").spawn()?;

    // Set up a 3D box.
    rec.log(
        "box",
        &[
            &rerun::Boxes3D::from_half_sizes([(4.0, 2.0, 1.0)])
                .with_fill_mode(rerun::FillMode::Solid) as &dyn AsComponents,
        ],
    )?;

    // Update only the rotation of the box.
    for deg in 0..=45 {
        let rad = truncated_radians((deg * 4) as f32);
        rec.log(
            "box",
            &rerun::Transform3D::new().with_rotation(rerun::RotationAxisAngle::new(
                [0.0, 1.0, 0.0],
                rerun::Angle::from_radians(rad),
            )),
        )?;
    }

    // Update only the position of the box.
    for t in 0..=50 {
        rec.log(
            "box",
            &rerun::Transform3D::new().with_translation([0.0, 0.0, t as f32 / 10.0]),
        )?;
    }

    // Update only the rotation of the box.
    for deg in 0..=45 {
        let rad = truncated_radians(((deg + 45) * 4) as f32);
        rec.log(
            "box",
            &rerun::Transform3D::new().with_rotation(rerun::RotationAxisAngle::new(
                [0.0, 1.0, 0.0],
                rerun::Angle::from_radians(rad),
            )),
        )?;
    }

    // Clear all of the box's attributes.
    rec.log("box", &rerun::Transform3D::clear_fields())?;

    Ok(())
}

fn truncated_radians(deg: f32) -> f32 {
    ((deg.to_radians() * 1000.0) as i32) as f32 / 1000.0
}
