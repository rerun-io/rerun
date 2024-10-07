//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p minimal_options -- --help
//! ```

use rerun::components::GraphEdgeUndirected;
use rerun::external::{log, re_log};

use rerun::{Color, GraphEdges, GraphNodes};

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

fn run(rec: &rerun::RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.set_time_sequence("frame", 0);
    rec.log(
        "kitchen/nodes",
        &GraphNodes::new(["sink", "fridge"])
            .with_colors([Color::from_rgb(255, 0, 0), Color::from_rgb(255, 255, 0)]),
    )?;

    rec.log("kitchen/nodes", &GraphNodes::new(["area0", "area1"]))?;
    rec.log(
        "kitchen/edges",
        &GraphEdges::new([("kitchen/nodes", "area0", "area1")]),
    )?;

    rec.set_time_sequence("frame", 1);
    rec.log("hallway/nodes", &GraphNodes::new(["area0"]))?;

    rec.set_time_sequence("frame", 2);
    rec.log("living/nodes", &GraphNodes::new(["table"]))?;
    rec.log(
        "living/nodes",
        &GraphNodes::new(["area0", "area1", "area2"]),
    )?;
    rec.log(
        "living/edges",
        &GraphEdges::new([
            ("living/nodes", "area0", "area1"),
            ("living/nodes", "area0", "area2"),
            ("living/nodes", "area1", "area2"),
        ]),
    )?;

    rec.log(
        "doors/edges",
        &GraphEdges::new([
            (("kitchen/nodes", "area0"), ("hallway/nodes", "area0")),
            (("hallway/nodes", "area0"), ("living/nodes", "area2")),
        ]),
    )?;

    rec.log(
        "edges",
        &GraphEdges::new([
            ("kitchen/nodes", "area0", "sink"),
            ("kitchen/nodes", "area1", "fridge"),
            ("living/nodes", "area1", "table"),
        ]),
    )?;

    Ok(())
}
