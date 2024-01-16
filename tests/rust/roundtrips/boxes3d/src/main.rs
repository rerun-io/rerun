//! Logs a `Box3D` archetype for roundtrip checks.

use rerun::{
    archetypes::Boxes3D,
    components::Rotation3D,
    datatypes::{Quaternion, RotationAxisAngle},
    transform::Angle,
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
        "boxes3d",
        &Boxes3D::from_half_sizes([[10., 9., 8.], [5., -5., 5.]])
            .with_centers([[0., 0., 0.], [-1., 1., -2.]])
            .with_rotations([
                Rotation3D::from(Quaternion::from_xyzw([0., 1., 2., 3.])),
                Rotation3D::from(RotationAxisAngle::new([0., 1., 2.], Angle::Degrees(45.))),
            ])
            .with_colors([0xAA0000CC, 0x00BB00DD])
            .with_labels(["hello", "friend"])
            .with_radii([0.1, 0.01])
            .with_class_ids([126, 127])
            .with_instance_keys([66, 666]),
    )?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_box3d")?;
    run(&rec, &args)
}
