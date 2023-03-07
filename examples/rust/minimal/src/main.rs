//! Demonstrates the most barebone usage of the Rerun SDK.

use rerun::{
    components::{ColorRGBA, Point3D},
    demo_util::grid,
    external::glam,
    MsgSender, Session, SessionBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rerun_enabled, recording_info) = SessionBuilder::new("minimal_rs").finalize();
    let session = if rerun_enabled {
        Session::buffered(recording_info)
    } else {
        Session::disabled()
    };

    let points = grid(glam::Vec3::splat(-5.0), glam::Vec3::splat(5.0), 10)
        .map(Point3D::from)
        .collect::<Vec<_>>();
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| ColorRGBA::from_rgb(v.x as u8, v.y as u8, v.z as u8))
        .collect::<Vec<_>>();

    MsgSender::new("my_points")
        .with_component(&points)?
        .with_component(&colors)?
        .send(&session)?;

    rerun::native_viewer::show(&session)?;

    Ok(())
}
