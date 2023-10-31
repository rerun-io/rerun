//! Log a batch of oriented bounding boxes.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_box3d_batch").spawn()?;

    rec.log(
        "batch",
        &rerun::Boxes3D::from_centers_and_half_sizes(
            [(2.0, 0.0, 0.0), (-2.0, 0.0, 0.0), (0.0, 0.0, 2.0)],
            [(2.0, 2.0, 1.0), (1.0, 1.0, 0.5), (2.0, 0.5, 1.0)],
        )
        .with_rotations([
            rerun::Rotation3D::IDENTITY,
            rerun::Quaternion::from_xyzw([0.0, 0.0, 0.382683, 0.923880]).into(), // 45 degrees around Z
            rerun::RotationAxisAngle::new((0.0, 1.0, 0.0), rerun::Angle::Degrees(30.0)).into(),
        ])
        .with_radii([0.025])
        .with_colors([
            rerun::Color::from_rgb(255, 0, 0),
            rerun::Color::from_rgb(0, 255, 0),
            rerun::Color::from_rgb(0, 0, 255),
        ])
        .with_labels(["red", "green", "blue"]),
    )?;

    Ok(())
}
