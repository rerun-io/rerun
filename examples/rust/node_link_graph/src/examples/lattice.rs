use core::num;

use itertools::Itertools;

use rerun::{GraphEdgesUndirected, GraphNodes};

use crate::Args;

pub fn run(args: &Args, num_nodes: usize) -> anyhow::Result<()> {
    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_lattice")?;

    let coordinates = (0..num_nodes).cartesian_product(0..num_nodes);

    let nodes = coordinates
        .clone()
        .enumerate()
        .map(|(i, _)| i.to_string())
        .collect::<Vec<_>>();
    rec.log_static("/nodes", &GraphNodes::new(nodes))?;

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
