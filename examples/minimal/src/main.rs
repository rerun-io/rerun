//! Demonstrates the most barebone usage of the Rerun SDK.

use rerun::external::glam;
use rerun::{ColorRGBA, MsgSender, Point3D, Session};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = Session::new();

    let points = grid(glam::Vec3::splat(-5.0), glam::Vec3::splat(5.0), 10)
        .map(Point3D::from)
        .collect::<Vec<_>>();
    let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
        .map(|v| ColorRGBA::from_rgb(v.x as u8, v.y as u8, v.z as u8))
        .collect::<Vec<_>>();

    MsgSender::new("my_point")
        .with_component(&points)?
        .with_component(&colors)?
        .send(&mut session)?;

    session.show()?;

    Ok(())
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * t + (1.0 - t) * b
}

fn linspace(a: f32, b: f32, steps: usize) -> impl Iterator<Item = f32> {
    (0..steps).map(move |t| lerp(a, b, t as f32 / (steps - 1) as f32))
}

fn grid(from: glam::Vec3, to: glam::Vec3, steps: usize) -> impl Iterator<Item = glam::Vec3> {
    linspace(from.z, to.z, steps).flat_map(move |z| {
        linspace(from.y, to.y, steps)
            .flat_map(move |y| linspace(from.x, to.x, steps).map(move |x| (x, y, z).into()))
    })
}
