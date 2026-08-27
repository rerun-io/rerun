//! Log an image as a 3D quad.

use ndarray::Array3;
use rand::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mesh3d_image")
        .spawn()?;

    // Simple gradient image
    let image = Array3::from_shape_fn((256, 256, 3), |(y, x, c)| match c {
        0 => x as u8,
        1 => y as u8,
        2 => 0,
        _ => unreachable!(),
    });

    let top_left = [1.0, 1.0, 1.0];
    let top_right = [1.0, 0.0, 1.0];
    let bottom_right = [1.0, 0.0, 0.0];
    let bottom_left = [1.0, 1.0, 0.0];
    let alpha = 255;

    // Inset by half a pixel so the opposite edges of the image don't leak onto the border.
    let height = image.shape()[0] as f32;
    let width = image.shape()[1] as f32;
    let (u0, v0) = (0.5 / width, 0.5 / height);
    let (u1, v1) = (1.0 - u0, 1.0 - v0);

    rec.log(
        "image",
        &rerun::Mesh3D::new([top_left, top_right, bottom_right, bottom_left])
            .with_vertex_texcoords([[u0, v0], [u1, v0], [u1, v1], [u0, v1]])
            .with_triangle_indices([[0, 2, 1], [0, 3, 2]])
            .with_albedo_texture_image(
                rerun::Image::from_color_model_and_tensor(
                    rerun::ColorModel::RGB,
                    image,
                )?,
            )
            .with_albedo_factor(rerun::Rgba32::from_unmultiplied_rgba(
                255, 255, 255, alpha,
            )),
    )?;

    // Example 3D data
    let mut rng = rand::rngs::SmallRng::seed_from_u64(1);
    let uniform = rand::distr::Uniform::new(0.1f32, 0.9f32)?;
    let normal_dist = rand_distr::Normal::new(0.0f32, 0.01f32)?;
    let color_dist = rand::distr::Uniform::new_inclusive(40u8, 254)?;

    let mut strips = Vec::with_capacity(20);
    let mut colors = Vec::with_capacity(20);
    for _ in 0..20 {
        let mut u = rng.sample(uniform);
        let mut v = rng.sample(uniform);
        let color = rerun::Color::from_rgb(
            rng.sample(color_dist),
            rng.sample(color_dist),
            rng.sample(color_dist),
        );

        let mut strip = Vec::with_capacity(40);
        for step in 0..40 {
            let t = step as f32 / (40 - 1) as f32;
            u = (u + rng.sample(normal_dist)).clamp(0.0, 1.0);
            v = (v + rng.sample(normal_dist)).clamp(0.0, 1.0);
            strip.push([1.0 - t, 1.0 - u, 1.0 - v]);
        }

        strips.push(strip);
        colors.push(color);
    }

    rec.log(
        "tracks",
        &rerun::LineStrips3D::new(strips)
            .with_colors(colors)
            .with_radii([0.002]),
    )?;

    rec.log(
        "time-axis",
        &rerun::Arrows3D::from_vectors([[1.0, 0.0, 0.0]])
            .with_origins([[0.0, 1.0, 0.0]])
            .with_labels(["Time"])
            .with_colors([[255, 255, 255]])
            .with_radii([0.005]),
    )?;

    Ok(())
}
