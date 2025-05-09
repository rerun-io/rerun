//! Logs a `SegmentationImage` archetype for roundtrip checks.

use image::GrayImage;
use rerun::{RecordingStream, archetypes::SegmentationImage};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let mut img = GrayImage::new(3, 2);

    // 3x2 image. Each pixel is incremented down each row
    for x in 0..3 {
        for y in 0..2 {
            img.put_pixel(x, y, image::Luma([(x + y * 3) as u8]));
        }
    }

    rec.log("segmentation_image", &SegmentationImage::try_from(img)?)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args
        .rerun
        .init("rerun_example_roundtrip_segmentation_image")?;
    run(&rec, &args)
}
