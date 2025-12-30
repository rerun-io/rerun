//! Example of using the blueprint APIs to configure Rerun.
//!
//! Usage:
//!   cargo run -p blueprint
//!   cargo run -p blueprint -- --skip-blueprint
//!   cargo run -p blueprint -- --auto-views

use rerun::blueprint::{Blueprint, ContainerLike, Grid, Spatial2DView};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// Don't send the blueprint
    #[clap(long)]
    skip_blueprint: bool,

    /// Automatically add views
    #[clap(long)]
    auto_views: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use clap::Parser;
    let args = Args::parse();

    let blueprint = if args.skip_blueprint {
        None
    } else {
        // Create a blueprint which includes 2 views each only showing 1 of the two rectangles.
        //
        // If auto_views is true, the blueprint will automatically add one of the heuristic
        // views, which will include the image and both rectangles.
        Some(
            Blueprint::new(Grid::new(vec![
                ContainerLike::from(
                    Spatial2DView::new("Rect 0")
                        .with_origin("/")
                        .with_contents(["image", "rect/0"]),
                ),
                ContainerLike::from(
                    Spatial2DView::new("Rect 1")
                        .with_origin("/")
                        .with_contents(["/**"]),
                ),
            ]))
            .with_auto_views(args.auto_views),
        )
    };

    let mut builder = rerun::RecordingStreamBuilder::new("rerun_example_blueprint");
    if let Some(blueprint) = blueprint {
        builder = builder.default_blueprint(blueprint);
    }
    let rec = builder.spawn()?;

    // Log an image with horizontal stripes
    let mut img = vec![0u8; 128 * 128 * 3];
    for i in 0..8 {
        for y in (i * 16 + 4)..(i * 16 + 12) {
            for x in 0..128 {
                img[(y * 128 + x) * 3 + 2] = 200; // Blue channel
            }
        }
    }
    rec.log("image", &rerun::Image::from_rgb24(img, [128, 128]))?;

    // Log rectangles at different times
    rec.set_time_sequence("frame", 10);
    rec.log(
        "rect/0",
        &rerun::Boxes2D::from_mins_and_sizes([(16.0, 16.0)], [(64.0, 64.0)])
            .with_labels(["Rect0"])
            .with_colors([rerun::Color::from_rgb(255, 0, 0)]),
    )?;

    rec.set_time_sequence("frame", 20);
    rec.log(
        "rect/1",
        &rerun::Boxes2D::from_mins_and_sizes([(48.0, 48.0)], [(64.0, 64.0)])
            .with_labels(["Rect1"])
            .with_colors([rerun::Color::from_rgb(0, 255, 0)]),
    )?;

    Ok(())
}
