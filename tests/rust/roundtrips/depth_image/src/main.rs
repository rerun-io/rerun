//! Logs an `Image` archetype for roundtrip checks.

use image::GrayImage;
use rerun::{archetypes::DepthImage, external::re_log, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let mut img = GrayImage::new(3, 2);

    // 3x2 image. Each pixel is i*j
    for x in 0..3 {
        for y in 0..2 {
            img.put_pixel(x, y, image::Luma([(x * y) as u8]));
        }
    }

    rec.log(
        "depth_image",
        &DepthImage::try_from(img)?.with_meter(1000.0),
    )?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_roundtrip_depth_image",
        default_enabled,
        move |rec| {
            run(&rec, &args).unwrap();
        },
    )
}
