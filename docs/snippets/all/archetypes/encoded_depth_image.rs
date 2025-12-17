//! Log an encoded depth image stored as a 16-bit PNG or RVL file

use rerun::external::anyhow;

fn main() -> anyhow::Result<()> {
    let args = _args;
    let Some(path) = args.get(1) else {
        anyhow::bail!("Usage: {} <path_to_depth_image.[png|rvl]>", args[0]);
    };

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_encoded_depth_image").spawn()?;

    let depth_blob = std::fs::read(path)?;
    let encoded_depth = rerun::EncodedDepthImage::new(depth_blob)
        .with_media_type(if path.ends_with(".png") {
            rerun::components::MediaType::PNG
        } else {
            rerun::components::MediaType::RVL
        })
        .with_meter(0.001_f32);

    rec.log("depth/encoded", &encoded_depth)?;

    Ok(())
}
