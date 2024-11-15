use rerun::external::ndarray;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_image_formats").spawn()?;

    // Simple gradient image
    let image = ndarray::Array3::from_shape_fn((256, 256, 3), |(y, x, c)| match c {
        0 => x as u8,
        1 => (x + y).min(255) as u8,
        2 => y as u8,
        _ => unreachable!(),
    });

    // RGB image
    rec.log(
        "image_rgb",
        &rerun::Image::from_color_model_and_tensor(rerun::ColorModel::RGB, image.clone())?,
    )?;

    // Green channel only (Luminance)
    rec.log(
        "image_green_only",
        &rerun::Image::from_color_model_and_tensor(
            rerun::ColorModel::L,
            image.slice(ndarray::s![.., .., 1]).to_owned(),
        )?,
    )?;

    // BGR image
    rec.log(
        "image_bgr",
        &rerun::Image::from_color_model_and_tensor(
            rerun::ColorModel::BGR,
            image.slice(ndarray::s![.., .., ..;-1]).to_owned(),
        )?,
    )?;

    // New image with Separate Y/U/V planes with 4:2:2 chroma downsampling
    let mut yuv_bytes = Vec::with_capacity(256 * 256 + 128 * 256 * 2);
    yuv_bytes.extend(std::iter::repeat(128).take(256 * 256)); // Fixed value for Y.
    yuv_bytes.extend((0..256).flat_map(|_y| (0..128).map(|x| x * 2))); // Gradient for U.
    yuv_bytes.extend((0..256).flat_map(|y| std::iter::repeat(y as u8).take(128))); // Gradient for V.
    rec.log(
        "image_yuv422",
        &rerun::Image::from_pixel_format(
            [256, 256],
            rerun::PixelFormat::Y_U_V16_FullRange,
            yuv_bytes,
        ),
    )?;

    Ok(())
}
