//! Demonstrates the most barebone usage of the Rerun SDK.

use rerun::{
    archetypes::Points3D, components::Color, demo_util::grid, external::glam,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `serve()` requires to have a running Tokio runtime in the current context.
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let rec = RecordingStreamBuilder::new("rerun_example_minimal_serve_rs").serve(
        "0.0.0.0",
        Default::default(),
        Default::default(),
        true,
    )?;

    let points = grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10);
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

    rec.log(
        "my_points",
        &Points3D::new(points).with_colors(colors).with_radii([0.5]),
    )?;

    eprintln!("Check your browser!");
    std::thread::sleep(std::time::Duration::from_secs(100000));

    Ok(())
}
