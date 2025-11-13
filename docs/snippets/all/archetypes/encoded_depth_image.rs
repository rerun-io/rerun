//! Log a PNG encoded depth image

use rerun::datatypes::ChannelDatatype;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_encoded_depth_image").spawn()?;

    const WIDTH: u32 = 64;
    const HEIGHT: u32 = 48;
    let format = rerun::components::ImageFormat::depth([WIDTH, HEIGHT], ChannelDatatype::U16);

    // Depth values are stored as millimeters in a 16-bit grayscale PNG.
    let depth_png = include_bytes!("encoded_depth.png");
    let encoded_depth = rerun::EncodedDepthImage::new(depth_png.to_vec(), format)
        .with_media_type(rerun::components::MediaType::png())
        .with_meter(0.001_f32);

    rec.log("depth/encoded", &encoded_depth)?;

    Ok(())
}
