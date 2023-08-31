//! Log different transforms between three arrows.
use rerun::{
    components::{Transform3D, Vector3D},
    datatypes::{Mat3x3, Vec3D},
    transform::{
        Angle, Rotation3D, RotationAxisAngle, Scale3D, TranslationAndMat3x3,
        TranslationRotationScale3D,
    },
    MsgSender, RecordingStreamBuilder,
};
use std::f32::consts::PI;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_transform").memory()?;

    let vector = Vector3D::from((0.0, 1.0, 0.0));

    MsgSender::new("base")
        .with_component(&[vector])?
        .send(&rec)?;

    MsgSender::new("base/translated")
        .with_component(&[Transform3D::new(TranslationAndMat3x3::new(
            Vec3D::from([1.0, 0.0, 0.0]),
            Mat3x3::IDENTITY,
        ))])?
        .send(&rec)?;

    MsgSender::new("base/translated")
        .with_component(&[vector])?
        .send(&rec)?;

    MsgSender::new("base/rotated_scaled")
        .with_component(&[Transform3D::new(TranslationRotationScale3D {
            translation: None,
            rotation: Some(Rotation3D::from(RotationAxisAngle::new(
                Vec3D::new(0.0, 0.0, 1.0),
                Angle::Radians(PI / 4.),
            ))),
            scale: Some(Scale3D::from(2.0)),
            ..Default::default()
        })])?
        .send(&rec)?;

    MsgSender::new("base/rotated_scaled")
        .with_component(&[vector])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
