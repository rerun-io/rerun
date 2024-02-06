//! Logs a `Transform3D` archetype for roundtrip checks.

use std::f32::consts::TAU;

use rerun::{
    archetypes::Transform3D,
    datatypes,
    datatypes::{
        Angle, RotationAxisAngle, Scale3D, TranslationAndMat3x3, TranslationRotationScale3D,
    },
    RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "translation_and_mat3x3/identity",
        &Transform3D::new(datatypes::Transform3D::TranslationAndMat3x3(
            TranslationAndMat3x3::IDENTITY,
        )), //
    )?;

    rec.log(
        "translation_and_mat3x3/translation",
        &Transform3D::new(datatypes::Transform3D::TranslationAndMat3x3(
            TranslationAndMat3x3::from_translation([1.0, 2.0, 3.0]).from_parent(),
        )), //
    )?;

    rec.log(
        "translation_and_mat3x3/rotation",
        &Transform3D::new(datatypes::Transform3D::TranslationAndMat3x3(
            TranslationAndMat3x3::from_mat3x3([[1.0, 4.0, 7.0], [2.0, 5.0, 8.0], [3.0, 6.0, 9.0]]),
        )),
    )?;

    rec.log(
        "translation_rotation_scale/identity",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::IDENTITY,
        )), //
    )?;

    rec.log(
        "translation_rotation_scale/translation_scale",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D {
                translation: Some([1.0, 2.0, 3.0].into()),
                scale: Some(Scale3D::Uniform(42.0)),
                ..Default::default()
            }
            .from_parent(),
        )), //
    )?;

    rec.log(
        "translation_rotation_scale/rigid",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::from_translation_rotation(
                [1.0, 2.0, 3.0],
                RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(0.5 * TAU)),
            ),
        )), //
    )?;

    rec.log(
        "translation_rotation_scale/affine",
        &Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::from_translation_rotation_scale(
                [1.0, 2.0, 3.0],
                RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(0.5 * TAU)),
                42.0,
            )
            .from_parent(),
        )), //
    )?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_transform3d")?;
    run(&rec, &args)
}
