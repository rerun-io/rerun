//! Logs an `Image` archetype for roundtrip checks.

use image::{Rgb, RgbImage};
use rerun::{archetypes::Image, datatypes::TensorId, external::re_log, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    // Need a deterministic id for round-trip tests. Used (10..26)
    let id = TensorId {
        uuid: core::array::from_fn(|i| (i + 10) as u8),
    };

    let mut img = RgbImage::new(3, 2);

    // 2x3x3 image. Red channel = x. Green channel = y. Blue channel = 128.
    for x in 0..3 {
        for y in 0..2 {
            img.put_pixel(x, y, Rgb([x as u8, y as u8, 128]));
        }
    }

    rec.log("image", &Image::try_from(img)?.with_id(id))?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_roundtrip_image",
        default_enabled,
        move |rec| {
            run(&rec, &args).unwrap();
        },
    )
}
