//! Use a blueprint to customize a graph view.

use rerun::blueprint::{
    archetypes as blueprint_archetypes, Blueprint, GraphView,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        GraphView::new("Graph")
            .with_origin("/")
            .with_visual_bounds(blueprint_archetypes::VisualBounds2D::new(
                rerun::datatypes::Range2D {
                    x_range: [-150.0, 150.0].into(),
                    y_range: [-50.0, 150.0].into(),
                },
            ))
            .with_background(
                blueprint_archetypes::GraphBackground::new()
                    .with_color([30, 10, 10]),
            ),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_graph_view")
        .with_blueprint(blueprint)
        .spawn()?;

    rec.log(
        "simple",
        &rerun::GraphNodes::new(["a", "b", "c"])
            .with_positions([(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)])
            .with_labels(["A", "B", "C"]),
    )?;

    Ok(())
}
