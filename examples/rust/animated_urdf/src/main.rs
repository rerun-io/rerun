//! An example of how to load and animate a URDF given some changing joint angles.
//!
//! Usage:
//! ```
//! cargo run -p animated_urdf
//! ```

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

use rerun::external::re_data_loader::{UrdfTree, urdf_joint_transform};
use rerun::external::{re_log, urdf_rs};

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_animated_urdf")?;
    run(&rec, &args)
}

fn run(rec: &rerun::RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let urdf_path = "examples/rust/animated_urdf/data/so100.urdf";

    // Log the URDF file one, as a static resource:
    rec.log_file_from_path(urdf_path, None, true)?;

    // Load the URDF tree structure into memory:
    let urdf = UrdfTree::from_file_path(urdf_path, None)?;

    // Animate:
    for step in 0..10000 {
        rec.set_time_sequence("step", step);
        for (joint_index, joint) in urdf.joints().enumerate() {
            if joint.joint_type == urdf_rs::JointType::Revolute {
                // Usually this angle would come from a measurement - here we just fake something:
                let dynamic_angle = emath::remap(
                    (step as f64 * (0.02 + joint_index as f64 / 100.0)).sin(),
                    -1.0..=1.0,
                    joint.limit.lower..=joint.limit.upper,
                );

                // Rerun loads the URDF transforms with child/parent frame relations.
                // In order to move a joint, we just need to log a new transform between two of those frames.
                //
                // We can use the `compute_transform3d` utility here, it handles origin + axis-angle rotation
                // and sets the parent/child frames according to the joint.
                let joint_transform =
                    urdf_joint_transform::compute_transform3d(joint, dynamic_angle, true)?;

                rec.log("/transforms", &joint_transform)?;
            }
        }
    }

    Ok(())
}
