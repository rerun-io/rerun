//! An example of how to load and animate a URDF given some changing joint angles.
//!
//! Usage:
//! ```
//! cargo run -p animate_urdf
//! ```

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

use rerun::external::{re_data_loader::UrdfTree, re_log, urdf_rs};

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_clock")?;
    run(&rec, &args)
}

fn run(rec: &rerun::RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let urdf_path = "examples/rust/animate_urdf/data/so100.urdf";

    // Log the URDF file one, as a static resource:
    rec.log_file_from_path(urdf_path, None, true)?;

    // Load the URDF tree structure into memory:
    let urdf = UrdfTree::from_file_path(urdf_path)?;

    // Animate:
    for step in 0..10000 {
        rec.set_time_sequence("step", step);
        for (joint_index, joint) in urdf.joints().enumerate() {
            if joint.joint_type == urdf_rs::JointType::Revolute {
                let fixed_axis = joint.axis.xyz.0;

                // Usually this angle would come from a measurement - here we just fake something:
                let dynamic_angle = emath::remap(
                    (step as f64 * (0.02 + joint_index as f64 / 100.0)).sin(),
                    -1.0..=1.0,
                    joint.limit.lower..=joint.limit.upper,
                );

                // NOTE: each join already has a fixed origin pose (logged with the URDF file),
                // and Rerun won't allow us to override or add to that transform here.
                // So instead we apply the dynamic rotation to the child link of the joint:
                let child_link = urdf.get_joint_child(joint);
                let link_path = urdf.get_link_path(child_link);
                rec.log(
                    link_path,
                    &rerun::Transform3D::update_fields().with_rotation(
                        rerun::RotationAxisAngle::new(fixed_axis, dynamic_angle as f32),
                    ),
                )?;
            }
        }
    }

    Ok(())
}
