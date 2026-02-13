//! Shows how to draw a graph with various node properties.
//!
//! Usage:
//! ```
//!  cargo run -p graph_lattice -- --connect
//! ```

use itertools::Itertools as _;
use rerun::external::re_log;
use rerun::{Color, GraphEdges, GraphNodes};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
pub struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

const NUM_NODES: usize = 10;

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_lattice")?;

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
