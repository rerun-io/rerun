//! Demonstrates the most barebone usage of the Rerun SDK.

use rerun::{
    components::{Color, Point3D, Radius},
    demo_util::grid,
    external::glam,
    MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `serve()` requires to have a running Tokio runtime in the current context.
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let rec_stream = RecordingStreamBuilder::new("rerun_example_minimal_serve_rs").serve(
        "0.0.0.0",
        Default::default(),
        Default::default(),
        true,
    )?;

    let points = grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10)
        .map(Point3D::from)
        .collect::<Vec<_>>();
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| Color::from_rgb(v.x as u8, v.y as u8, v.z as u8))
        .collect::<Vec<_>>();

    MsgSender::new("my_points")
        .with_component(&points)?
        .with_component(&colors)?
        .with_splat(Radius(0.5))?
        .send(&rec_stream)?;

    eprintln!("Check your browser!");
    std::thread::sleep(std::time::Duration::from_secs(100000));

    Ok(())
}
