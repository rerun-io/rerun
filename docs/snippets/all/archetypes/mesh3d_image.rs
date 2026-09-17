//! Log an image as a 3D quad.
//!
//! See also `GridMap` for an alternative way to show image data like robot
//! maps in 3D, or `Pinhole` to log images under a camera projection.

use ndarray::Array3;

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

    Ok(())
}
