use itertools::Itertools;

use rerun::{Color, GraphEdgesUndirected, GraphNodes};

use crate::Args;

pub fn run(args: &Args, num_nodes: usize) -> anyhow::Result<()> {
    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_lattice")?;

    let coordinates = (0..num_nodes).cartesian_product(0..num_nodes);

    let (nodes, colors): (Vec<_>, Vec<_>) = coordinates
        .clone()
        .enumerate()
        .map(|(i, (x, y))| {
            let r = ((x as f32 / (num_nodes - 1) as f32) * 255.0).round() as u8;
            let g = ((y as f32 / (num_nodes - 1) as f32) * 255.0).round() as u8;
            (i.to_string(), Color::from_rgb(r, g, 0))
        })
        .unzip();
    rec.log_static("/nodes", &GraphNodes::new(nodes).with_colors(colors))?;

    let mut edges = Vec::new();
    for (x, y) in coordinates {
        if y > 0 {
            let source = (y - 1) * num_nodes + x;
            let target = y * num_nodes + x;
            edges.push(("/nodes", source.to_string(), target.to_string()));
        }
        if x > 0 {
            let source = y * num_nodes + (x - 1);
            let target = y * num_nodes + x;
            edges.push(("/nodes", source.to_string(), target.to_string()));
        }
    }

    rec.log_static("/edges", &GraphEdgesUndirected::new(edges))?;
    Ok(())
}
