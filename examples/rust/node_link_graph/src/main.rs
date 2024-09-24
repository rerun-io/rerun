//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p minimal_options -- --help
//! ```

use rerun::datatypes::GraphNodeId;
use rerun::external::re_log;

use rerun::demo_util::grid;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    #[clap(long, default_value = "10")]
    num_points_per_axis: usize,

    #[clap(long, default_value = "10.0")]
    radius: f32,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_node_link_graph")?;
    run(&rec, &args)
}

fn run(rec: &rerun::RecordingStream, args: &Args) -> anyhow::Result<()> {
    rec.set_time_sequence("frame", 0);
    _ = rec.log(
        "graph",
        &rerun::GraphNodes::new([1, 2, 3]).with_labels(["a", "b", "c"]),
    );
    _ = rec.log(
        "graph",
        &rerun::GraphEdges::new([
            // TODO(grtlr): Provide a nicer way to create these.
            [GraphNodeId(1), GraphNodeId(2)],
            [GraphNodeId(2), GraphNodeId(3)],
            [GraphNodeId(1), GraphNodeId(3)],
        ]),
    );

    rec.set_time_sequence("frame", 1);
    _ = rec.log(
        "graph/level-1",
        &rerun::GraphNodes::new([4, 5, 6]).with_labels(["d", "e", "f"]),
    );
    _ = rec.log(
        "graph/level-1",
        &rerun::GraphEdges::new([
            [GraphNodeId(3), GraphNodeId(4)],
            [GraphNodeId(4), GraphNodeId(5)],
            [GraphNodeId(5), GraphNodeId(6)],
        ]),
    );

    Ok(())
}
