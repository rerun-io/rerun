//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p minimal_options -- --help
//! ```

use rerun::components::GraphEdge;
use rerun::external::re_log;

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
        "kitchen/objects",
        &GraphNodes::new(["sink", "fridge"])
            .with_colors([Color::from_rgb(255, 0, 0), Color::from_rgb(255, 255, 0)]),
    )?;
    rec.log("kitchen/areas", &GraphNodes::new(["area0", "area1"]))?;
    rec.log("kitchen/areas", &GraphEdges::new([("area0", "area1")]))?;

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
        &GraphEdges::new([("area0", "area1"), ("area0", "area2"), ("area1", "area2")]),
    )?;

    rec.log(
        "doors",
        &GraphEdges::new([
            GraphEdge::new("area1", "area0")
                .with_source_in("kitchen/areas")
                .with_target_in("hallway/areas"),
            GraphEdge::new("area0", "area2")
                .with_source_in("hallway/areas")
                .with_target_in("living/areas"),
        ]),
    )?;

    rec.log(
        "reachable",
        &GraphEdges::new([
            GraphEdge::new("area0", "sink")
                .with_source_in("kitchen/areas")
                .with_target_in("kitchen/objects"),
            GraphEdge::new("area1", "fridge")
                .with_source_in("kitchen/areas")
                .with_target_in("kitchen/objects"),
                GraphEdge::new("area1", "table")
                .with_source_in("living/areas")
                .with_target_in("living/objects"),
        ]),
    )?;

    Ok(())
}
