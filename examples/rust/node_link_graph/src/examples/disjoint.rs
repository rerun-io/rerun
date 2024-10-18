use rerun::GraphNodes;

use crate::Args;

pub fn run(args: &Args, num_nodes: usize) -> anyhow::Result<()> {
    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_disjoint")?;

    let nodes = (0..num_nodes)
        .map(|i| format!("node{}", i))
        .collect::<Vec<_>>();

    rec.log_static("/nodes", &GraphNodes::new(nodes))?;
    Ok(())
}
