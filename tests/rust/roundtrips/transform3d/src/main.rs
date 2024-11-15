//! Logs a `Transform3D` archetype for roundtrip checks.

use std::f32::consts::TAU;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &rerun::RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "transform/translation",
        &rerun::Transform3D::from_translation([1.0, 2.0, 3.0])
            .with_relation(rerun::TransformRelation::ChildFromParent),
    )?;

    rec.log(
        "transform/rotation",
        &rerun::Transform3D::from_mat3x3([[1.0, 4.0, 7.0], [2.0, 5.0, 8.0], [3.0, 6.0, 9.0]]),
    )?;

    rec.log(
        "transform/translation_scale",
        &rerun::Transform3D::from_translation_scale([1.0, 2.0, 3.0], rerun::Scale3D::uniform(42.0))
            .with_relation(rerun::TransformRelation::ChildFromParent),
    )?;

    rec.log(
        "transform/rigid",
        &rerun::Transform3D::from_translation_rotation(
            [1.0, 2.0, 3.0],
            rerun::RotationAxisAngle::new([0.2, 0.2, 0.8], rerun::Angle::from_radians(0.5 * TAU)),
        ),
    )?;

    rec.log(
        "transform/affine",
        &rerun::Transform3D::from_translation_rotation_scale(
            [1.0, 2.0, 3.0],
            rerun::RotationAxisAngle::new([0.2, 0.2, 0.8], rerun::Angle::from_radians(0.5 * TAU)),
            42.0,
        )
        .with_relation(rerun::TransformRelation::ChildFromParent),
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
