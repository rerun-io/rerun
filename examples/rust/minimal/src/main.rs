//! Demonstrates the most barebone usage of the Rerun SDK.
use rerun::RecordingStreamBuilder;
use rerun::{demo_util::grid, external::glam};

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

fn main() -> Result {
    let rec = RecordingStreamBuilder::new("tmp").stdout()?;

    for i in 0..10000 {
        let points = grid(
            glam::Vec3::splat(-(i as f32) / 20.0),
            glam::Vec3::splat((i as f32) / 20.0),
            i / 200,
        );
        let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), i / 200)
            .map(|v| rerun::Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

        rec.log("points", &rerun::Points3D::new(points).with_colors(colors))?;
    }

    Ok(())
}
