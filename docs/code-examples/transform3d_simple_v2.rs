//! Log some transforms.

use rerun::{
    archetypes::Transform3D,
    components::Vector3D,
    datatypes::{
        Angle, Mat3x3, RotationAxisAngle, Scale3D, TranslationAndMat3x3, TranslationRotationScale3D,
    },
    MsgSender, RecordingStreamBuilder,
};
use std::f32::consts::PI;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("rerun_example_transform3d").memory()?;

    let vector = Vector3D::from((0.0, 1.0, 0.0));

    MsgSender::new("base")
        .with_component(&[vector])?
        .send(&rec_stream)?;

    MsgSender::from_archetype(
        "base/translated",
        &Transform3D::new(TranslationAndMat3x3::new([1.0, 0.0, 0.0], Mat3x3::IDENTITY)),
    )?
    .send(&rec_stream)?;

    MsgSender::new("base/translated")
        .with_component(&[vector])?
        .send(&rec_stream)?;

    MsgSender::from_archetype(
        "base/rotated_scaled",
        &Transform3D::new(TranslationRotationScale3D {
            rotation: Some(RotationAxisAngle::new([0.0, 0.0, 1.0], Angle::Radians(PI / 4.)).into()),
            scale: Some(Scale3D::from(2.0)),
            ..Default::default()
        }),
    )?
    .send(&rec_stream)?;

    MsgSender::new("base/rotated_scaled")
        .with_component(&[vector])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
