//! Log a batch of oriented bounding boxes.
use rerun::{
    components::Color, Angle, Boxes3D, Quaternion, RecordingStreamBuilder, Rotation3D,
    RotationAxisAngle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_box3d_batch").memory()?;

    rec.log(
        "batch",
        &Boxes3D::from_centers_and_half_sizes(
            [(2.0, 0.0, 0.0), (-2.0, 0.0, 0.0), (0.0, 0.0, 2.0)],
            [(2.0, 2.0, 1.0), (1.0, 1.0, 0.5), (2.0, 0.5, 1.0)],
        )
        .with_rotations([
            Rotation3D::IDENTITY,
            Quaternion::from_xyzw([0.0, 0.0, 0.382683, 0.923880]).into(), // 45 degrees around Z
            RotationAxisAngle::new((0.0, 1.0, 0.0), Angle::Degrees(30.0)).into(),
        ])
        .with_radii([0.025])
        .with_colors([
            Color::from_rgb(255, 0, 0),
            Color::from_rgb(0, 255, 0),
            Color::from_rgb(0, 0, 255),
        ])
        .with_labels(["red", "green", "blue"]),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
