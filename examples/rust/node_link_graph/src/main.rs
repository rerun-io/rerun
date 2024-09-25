//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p minimal_options -- --help
//! ```

use rerun::external::re_log;
use rerun::{components, datatypes, EntityPath};

use rerun::demo_util::grid;
use rerun::{GraphEdges, GraphNodes};

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
    rec.log("kitchen/objects", &GraphNodes::new(["sink", "fridge"]))?;
    rec.log("kitchen/areas", &GraphNodes::new(["area0", "area1"]))?;
    rec.log(
        "kitchen/areas",
        &GraphEdges::new([components::GraphEdge::new("area0", "area1")]),
    )?;

    rec.set_time_sequence("frame", 1);
    rec.log("hallway/areas", &GraphNodes::new(["area0"]))?;

    rec.set_time_sequence("frame", 2);
    rec.log("living/objects", &GraphNodes::new(["table"]))?;
    rec.log(
        "living/areas",
        &GraphNodes::new(["area0", "area1", "area2"]),
    )?;
    rec.log(
        "living/areas",
        &GraphEdges::new([
            components::GraphEdge::new("area0", "area1"),
            components::GraphEdge::new("area0", "area2"),
            components::GraphEdge::new("area1", "area2"),
        ]),
    )?;

    rec.log(
        "doors",
        &GraphEdges::new([components::GraphEdge::new_global(
            (
                datatypes::EntityPath::from("kitchen/areas"),
                datatypes::GraphNodeId::from("area1"),
            ),
            (
                datatypes::EntityPath::from("kitchen/areas"),
                datatypes::GraphNodeId::from("area1"),
            ),
        )]),
    )?;

    Ok(())
}
