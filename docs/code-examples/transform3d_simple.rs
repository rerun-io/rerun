//! Log some transforms.

use std::f32::consts::TAU;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_transform3d").memory()?;

    rec.log(
        "base",
        &rerun::Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
    )?;

    rec.log(
        "base/translated",
        &rerun::Transform3D::from_translation([1.0, 0.0, 0.0]),
    )?;

    rec.log(
        "base/translated",
        &rerun::Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
    )?;

    rec.log(
        "base/rotated_scaled",
        &rerun::Transform3D::from_rotation_scale(
            rerun::RotationAxisAngle::new([0.0, 0.0, 1.0], rerun::Angle::Radians(TAU / 8.0)),
            rerun::Scale3D::from(2.0),
        ),
    )?;

    rec.log(
        "base/rotated_scaled",
        &rerun::Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
