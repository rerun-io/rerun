//! Logs a `Transform3D` archetype for roundtrip checks.

use std::f32::consts::PI;

use rerun::{
    archetypes::Transform3D,
    datatypes,
    datatypes::{
        Angle, RotationAxisAngle, Scale3D, TranslationAndMat3x3, TranslationRotationScale3D,
    },
    external::re_log,
    MsgSender, RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec_stream: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    MsgSender::from_archetype(
        "translation_and_mat3x3/identity",
        &Transform3D::new(datatypes::Transform3D::TranslationAndMat3X3(
            TranslationAndMat3x3::IDENTITY,
        )), //
    )?
    .send(rec_stream)?;

    MsgSender::from_archetype(
        "translation_and_mat3x3/translation",
        &Transform3D::new(datatypes::Transform3D::TranslationAndMat3X3(
            TranslationAndMat3x3::translation([1.0, 2.0, 3.0]).from_parent(),
        )), //
    )?
    .send(rec_stream)?;

    MsgSender::from_archetype(
        "translation_and_mat3x3/rotation",
        &Transform3D::new(datatypes::Transform3D::TranslationAndMat3X3(
            TranslationAndMat3x3::rotation([[1.0, 4.0, 7.0], [2.0, 5.0, 8.0], [3.0, 6.0, 9.0]]),
        )),
    )?
    .send(rec_stream)?;

    MsgSender::from_archetype(
        "translation_rotation_scale/identity",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::IDENTITY,
        )), //
    )?
    .send(rec_stream)?;

    MsgSender::from_archetype(
        "translation_rotation_scale/translation_scale",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D {
                translation: Some([1.0, 2.0, 3.0].into()),
                scale: Some(Scale3D::Uniform(42.0)),
                ..Default::default()
            }
            .from_parent(),
        )), //
    )?
    .send(rec_stream)?;

    MsgSender::from_archetype(
        "translation_rotation_scale/rigid",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::rigid(
                [1.0, 2.0, 3.0],
                RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(PI)),
            ),
        )), //
    )?
    .send(rec_stream)?;

    MsgSender::from_archetype(
        "translation_rotation_scale/affine",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::affine(
                [1.0, 2.0, 3.0],
                RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(PI)),
                42.0,
            )
            .from_parent(),
        )), //
    )?
    .send(rec_stream)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun-example-roundtrip_transform3d",
        default_enabled,
        move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        },
    )
}
