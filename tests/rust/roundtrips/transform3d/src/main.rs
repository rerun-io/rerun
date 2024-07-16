//! Logs a `Transform3D` archetype for roundtrip checks.

use std::f32::consts::TAU;

use rerun::{
    archetypes::Transform3D,
    datatypes::{Angle, RotationAxisAngle, Scale3D},
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
        "transform/translation",
        &Transform3D::from_translation([1.0, 2.0, 3.0]).from_parent(),
    )?;

    rec.log(
        "transform/rotation",
        &Transform3D::from_mat3x3([[1.0, 4.0, 7.0], [2.0, 5.0, 8.0], [3.0, 6.0, 9.0]]),
    )?;

    rec.log(
        "transform/translation_scale",
        &Transform3D::from_translation_scale([1.0, 2.0, 3.0], Scale3D::Uniform(42.0)).from_parent(),
    )?;

    rec.log(
        "transform/rigid",
        &Transform3D::from_translation_rotation(
            [1.0, 2.0, 3.0],
            RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(0.5 * TAU)),
        ),
    )?;

    rec.log(
        "transform/affine",
        &Transform3D::from_translation_rotation_scale(
            [1.0, 2.0, 3.0],
            RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(0.5 * TAU)),
            42.0,
        )
        .from_parent(),
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
