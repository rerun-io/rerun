//! Visual test for how we project 3D into 2D using Pinhole.
//!
//! ```
//! cargo run -p test_pinhole_projection
//! ```

use rerun::{RecordingStream, external::re_log};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_test_pinhole_projection")?;
    run(&rec)
}

fn run(rec: &RecordingStream) -> anyhow::Result<()> {
    const DESCRIPTION: &str = "\
    Add `/world/points` to the 2D image space (projected).\n\nn\
    The five points should project onto the corners and center of the image.\n\n\
    Only the top-right point should be undistorted (circular).
    ";
    rec.log_static(
        "description",
        &rerun::TextDocument::from_markdown(DESCRIPTION),
    )?;

    rec.log_static("world", &rerun::ViewCoordinates::RIGHT_HAND_Y_DOWN())?;

    const W: u32 = 2000;
    const H: u32 = 1000;

    let focal_length = [H as f32, H as f32];

    rec.log(
        "world/camera/image",
        &rerun::Pinhole::from_focal_length_and_resolution(focal_length, [W as f32, H as f32])
            // We put the principal point in the unusual top-right corner
            .with_principal_point([W as f32, 0.0]),
    )?;

    let mut depth_image: image::ImageBuffer<image::Luma<u16>, Vec<u16>> =
        image::ImageBuffer::new(W, H);
    for y in 0..H {
        for x in 0..W {
            let depth = 2000; // TODO(emilk): we could parameterize this over x/y to make it more interesting.
            depth_image.put_pixel(x, y, image::Luma([depth]));
        }
    }

    rec.log(
        "world/camera/image/depth",
        &rerun::DepthImage::try_from(depth_image)?.with_meter(1000.0),
    )?;

    rec.log(
        "world/camera/image/bottom_right",
        &rerun::Points2D::new([[W as f32, H as f32]]),
    )?;

    rec.log(
        "world/points",
        &rerun::Points3D::new([
            [0.0, 0.0, 1.0],
            [-2.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [-2.0, 1.0, 1.0],
            [-1.0, 0.5, 1.0],
        ])
        .with_radii([0.05]),
    )?;

    Ok(())
}
