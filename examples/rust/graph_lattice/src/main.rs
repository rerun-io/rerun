//! Shows how to draw a graph with various node properties.

use itertools::Itertools as _;
use rerun::{Color, GraphEdges, GraphNodes};

const NUM_NODES: usize = 10;

fn main() -> anyhow::Result<()> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_graph_lattice").spawn()?;

    let coordinates = (0..NUM_NODES).cartesian_product(0..NUM_NODES);

    let (nodes, colors): (Vec<_>, Vec<_>) = coordinates
        .clone()
        .enumerate()
        .map(|(i, (x, y))| {
            let r = ((x as f32 / (NUM_NODES - 1) as f32) * 255.0).round() as u8;
            let g = ((y as f32 / (NUM_NODES - 1) as f32) * 255.0).round() as u8;
            (i.to_string(), Color::from_rgb(r, g, 0))
        })
        .unzip();

    rec.log_static(
        "/lattice",
        &GraphNodes::new(nodes)
            .with_colors(colors)
            .with_labels(coordinates.clone().map(|(x, y)| format!("({x}, {y})"))),
    )?;

    let mut edges = Vec::new();
    for (x, y) in coordinates {
        if y > 0 {
            let source = (y - 1) * NUM_NODES + x;
            let target = y * NUM_NODES + x;
            edges.push((source.to_string(), target.to_string()));
        }
        if x > 0 {
            let source = y * NUM_NODES + (x - 1);
            let target = y * NUM_NODES + x;
            edges.push((source.to_string(), target.to_string()));
        }
    }

    rec.log_static("/lattice", &GraphEdges::new(edges).with_directed_edges())?;

    Ok(())
}
