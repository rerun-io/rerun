//! Demonstrates the most barebone usage of the Rerun SDK.

use std::f32::consts::TAU;

use rerun::{external::glam::Quat, HalfSizes3D, Position3D, Quaternion, Rgba32};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_splat").spawn()?;

    rec.log(
        "a",
        &rerun::Points3D::new([Position3D::new(0.0, 0.0, 0.0); 1])
            .with_colors([Rgba32::WHITE])
            .with_radii([0.5])
            .with_rotations([Quaternion::IDENTITY; 1])
            .with_scales([HalfSizes3D::new(1.0, 2.0, 0.01); 1]),
    )?;
    rec.log(
        "b",
        &rerun::Points3D::new([Position3D::new(4.0, 0.0, 0.0); 1])
            .with_colors([Rgba32::BLACK])
            .with_radii([0.5])
            .with_rotations([Quat::from_rotation_x(TAU / 12.0); 1])
            .with_scales([HalfSizes3D::new(1.0, 2.0, 0.01); 1]),
    )?;

    Ok(())
}
