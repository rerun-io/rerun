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

use rerun::components::Translation3D;
use rerun::external::re_data_loader::UrdfTree;
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
    rec.log_file_from_path(urdf_path, None, None, true)?;

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

                // Compute the full rotation for this joint.
                // TODO(michael): we could make this a bit nicer with a better URDF utility.
                let rotation = glam::Quat::from_euler(
                    glam::EulerRot::XYZ,
                    joint.origin.rpy[0] as f32,
                    joint.origin.rpy[1] as f32,
                    joint.origin.rpy[2] as f32,
                ) * glam::Quat::from_axis_angle(
                    glam::Vec3::new(
                        fixed_axis[0] as f32,
                        fixed_axis[1] as f32,
                        fixed_axis[2] as f32,
                    ),
                    dynamic_angle as f32,
                );

                // Rerun loads the URDF transforms with child/parent frame relations.
                // In order to move a joint, we just need to log a new transform between two of those frames.
                rec.log(
                    "/transforms",
                    &rerun::Transform3D::from_rotation(rerun::Quaternion::from_xyzw(
                        rotation.to_array(),
                    ))
                    .with_translation(Translation3D::from(joint.origin.xyz.0))
                    .with_parent_frame(joint.parent.link.clone())
                    .with_child_frame(joint.child.link.clone()),
                )?;
            }
        }
    }

    Ok(())
}
