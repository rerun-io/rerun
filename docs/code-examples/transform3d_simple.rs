//! Log some transforms.

use rerun::{
    archetypes::Transform3D, Angle, Arrows3D, RecordingStreamBuilder, RotationAxisAngle, Scale3D,
    TranslationRotationScale3D,
};
use std::f32::consts::TAU;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_transform3d").memory()?;

    rec.log(
        "base",
        &Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
    )?;

    rec.log(
        "base/translated",
        &Transform3D::new(TranslationRotationScale3D::translation([1.0, 0.0, 0.0])),
    )?;

    rec.log(
        "base/translated",
        &Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
    )?;

    rec.log(
        "base/rotated_scaled",
        &Transform3D::new(TranslationRotationScale3D {
            rotation: Some(
                RotationAxisAngle::new([0.0, 0.0, 1.0], Angle::Radians(TAU / 8.0)).into(),
            ),
            scale: Some(Scale3D::from(2.0)),
            ..Default::default()
        }),
    )?;

    rec.log(
        "base/rotated_scaled",
        &Arrows3D::from_vectors([(0.0, 1.0, 0.0)]).with_origins([(0.0, 0.0, 0.0)]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
