//! Log some transforms.

use rerun::{
    archetypes::{Arrows3D, Transform3D},
    datatypes::{
        Angle, Mat3x3, RotationAxisAngle, Scale3D, TranslationAndMat3x3, TranslationRotationScale3D,
    },
    RecordingStreamBuilder,
};
use std::f32::consts::PI;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_transform3d").memory()?;

    rec.log("base", &Arrows3D::new([(0.0, 1.0, 0.0)]))?;

    rec.log(
        "base/translated",
        &Transform3D::new(TranslationAndMat3x3::new([1.0, 0.0, 0.0], Mat3x3::IDENTITY)),
    )?;

    rec.log("base/translated", &Arrows3D::new([(0.0, 1.0, 0.0)]))?;

    rec.log(
        "base/rotated_scaled",
        &Transform3D::new(TranslationRotationScale3D {
            rotation: Some(RotationAxisAngle::new([0.0, 0.0, 1.0], Angle::Radians(PI / 4.)).into()),
            scale: Some(Scale3D::from(2.0)),
            ..Default::default()
        }),
    )?;

    rec.log("base/rotated_scaled", &Arrows3D::new([(0.0, 1.0, 0.0)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
