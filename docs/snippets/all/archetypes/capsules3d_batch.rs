//! Log a batch of capsules.

use rerun::external::glam::vec3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_capsule3d_batch").spawn()?;

    rec.log(
        "capsules",
        &rerun::Capsules3D::from_lengths_and_radii(
            [0.0, 2.0, 4.0, 6.0, 8.0],
            [1.0, 0.5, 0.5, 0.5, 1.0],
        )
        .with_colors([
            rerun::Color::from_rgb(255, 0, 0),
            rerun::Color::from_rgb(188, 188, 0),
            rerun::Color::from_rgb(0, 255, 0),
            rerun::Color::from_rgb(0, 188, 188),
            rerun::Color::from_rgb(0, 0, 255),
        ])
        .with_translations([
            vec3(0., 0., 0.),
            vec3(2., 0., 0.),
            vec3(4., 0., 0.),
            vec3(6., 0., 0.),
            vec3(8., 0., 0.),
        ])
        .with_rotation_axis_angles((0..5).map(|i| {
            rerun::RotationAxisAngle::new(
                [1.0, 0.0, 0.0],
                rerun::Angle::from_degrees(i as f32 * -22.5),
            )
        })),
    )?;

    Ok(())
}
