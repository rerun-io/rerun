//! Logs a point cloud and a perspective camera looking at it.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_pinhole_perspective").spawn()?;

    let fov_y = std::f32::consts::FRAC_PI_4;
    let aspect_ratio = 1.7777778;
    rec.log(
        "world/cam",
        &rerun::Pinhole::from_fov_and_aspect_ratio(fov_y, aspect_ratio)
            .with_camera_xyz(rerun::components::ViewCoordinates::RUB)
            .with_image_plane_distance(0.1)
            .with_color(rerun::Color::from_rgb(255, 128, 0))
            .with_line_width(0.003),
    )?;

    rec.log(
        "world/points",
        &rerun::Points3D::new([(0.0, 0.0, -0.5), (0.1, 0.1, -0.5), (-0.1, -0.1, -0.5)])
            .with_radii([0.025]),
    )?;

    Ok(())
}
