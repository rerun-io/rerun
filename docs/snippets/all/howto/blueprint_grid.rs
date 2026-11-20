//! Lay out two 2D views in a grid, with per-view contents, defaults, and overrides.

use rerun::blueprint::components::PanelState;
use rerun::blueprint::{
    Blueprint, ContainerLike, Grid, SelectionPanel, Spatial2DView,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(Grid::new(vec![
        ContainerLike::from(
            Spatial2DView::new("Image and first rect")
                .with_origin("/")
                .with_contents(["image", "rect/0"]),
        ),
        ContainerLike::from(
            Spatial2DView::new("Everything")
                .with_origin("/")
                .with_contents(["/**"])
                .with_defaults(
                    &rerun::Boxes2D::update_fields().with_radii([2.0]),
                )
                .with_override(
                    "rect/0",
                    &rerun::Boxes2D::update_fields().with_radii([1.0]),
                ),
        ),
    ]))
    .with_selection_panel(SelectionPanel::from_state(PanelState::Collapsed));

    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_blueprint_grid")
            .with_blueprint(blueprint)
            .spawn()?;

    rec.log(
        "rect/0",
        &rerun::Boxes2D::from_mins_and_sizes([(16.0, 16.0)], [(64.0, 64.0)]),
    )?;

    Ok(())
}
