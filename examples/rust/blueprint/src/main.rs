use rerun::{
    blueprint::{
        Blueprint, BlueprintPanel, ContainerLike, Grid, SelectionPanel, Spatial2DView, TimePanel,
    },
    external::{
        re_log_types::TimeInt,
        re_sdk_types::blueprint::components::{LoopMode, PanelState, PlayState},
    },
};

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
                        .with_contents(["/**"])
                        .with_defaults(&rerun::Boxes2D::update_fields().with_radii([2.0]))
                        .with_override(
                            "rect/0",
                            &rerun::Boxes2D::update_fields().with_radii([1.0]),
                        ),
                ),
            ]))
            .with_auto_views(args.auto_views)
            .with_blueprint_panel(BlueprintPanel::from_state(PanelState::Collapsed))
            .with_selection_panel(SelectionPanel::from_state(PanelState::Collapsed))
            .with_time_panel(
                TimePanel::new()
                    .with_state(PanelState::Collapsed)
                    .with_timeline("custom")
                    .with_time_selection(TimeInt::new_temporal(10)..=TimeInt::new_temporal(25))
                    .with_loop_mode(LoopMode::Selection)
                    .with_play_state(PlayState::Playing),
            ),
        )
    };

    let (rec, _) = match blueprint {
        Some(blueprint) => args
            .rerun
            .init_with_blueprint("rerun_example_blueprint", blueprint)?,
        None => args.rerun.init("rerun_example_blueprint")?,
    };

    rec.set_time_sequence("custom", 0);

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
    rec.set_time_sequence("custom", 10);
    rec.log(
        "rect/0",
        &rerun::Boxes2D::from_mins_and_sizes([(16.0, 16.0)], [(64.0, 64.0)])
            .with_labels(["Rect0"])
            .with_colors([rerun::Color::from_rgb(255, 0, 0)]),
    )?;

    rec.set_time_sequence("custom", 20);
    rec.log(
        "rect/1",
        &rerun::Boxes2D::from_mins_and_sizes([(48.0, 48.0)], [(64.0, 64.0)])
            .with_labels(["Rect1"])
            .with_colors([rerun::Color::from_rgb(0, 255, 0)]),
    )?;

    Ok(())
}
