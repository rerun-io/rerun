//! Connect to the viewer and log some data.

use rerun::{demo_util::grid, external::glam};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_attach_sinks").set_sinks((
        // Connect to a local viewer using the default URL.
        rerun::sink::GrpcSink::default(),
        // Write data to a `data.rrd` file in the current directory.
        rerun::sink::FileSink::new("data.rrd")?,
    ))?;

    // Create some data using the `grid` utility function.
    let points = grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10);
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| rerun::Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    rec.log(
        "my_points",
        &rerun::Points3D::new(points)
            .with_colors(colors)
            .with_radii([0.5]),
    )?;

    Ok(())
}
