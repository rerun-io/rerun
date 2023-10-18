//! Logs an `Image` archetype for roundtrip checks.

use half::f16;
use image::{Rgb, RgbImage};
use ndarray::{Array, ShapeBuilder};
use rerun::{archetypes::Image, external::re_log, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let mut img = RgbImage::new(3, 2);

    // h=2 w=3 c=3 image. Red channel = x. Green channel = y. Blue channel = 128.
    for x in 0..3 {
        for y in 0..2 {
            img.put_pixel(x, y, Rgb([x as u8, y as u8, 128]));
        }
    }

    rec.log("image", &Image::try_from(img)?)?;

    let mut array_image = Array::<f16, _>::default((4, 5).f());

    // h=4, w=5 mono image. Pixel = x * y * 123.4
    for y in 0..4 {
        for x in 0..5 {
            *array_image.get_mut((y, x)).unwrap() = f16::from_f32(x as f32 * y as f32 * 123.4);
        }
    }

    rec.log("image_f16", &Image::try_from(array_image)?)?;

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
