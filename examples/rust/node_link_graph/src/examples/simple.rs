use rerun::{Color, GraphEdgesDirected, GraphEdgesUndirected, GraphNodes};

use crate::Args;

pub fn run(args: &Args) -> anyhow::Result<()> {
    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_simple")?;

    rec.set_time_sequence("frame", 0);
    rec.log(
        "kitchen/objects",
        &GraphNodes::new(["sink", "fridge"])
            .with_colors([Color::from_rgb(255, 0, 0), Color::from_rgb(255, 255, 0)]),
    )?;

    rec.log("kitchen/nodes", &GraphNodes::new(["area0", "area1"]))?;
    rec.log(
        "kitchen/edges",
        &GraphEdgesDirected::new([("kitchen/nodes", "area0", "area1")]),
    )?;

    rec.set_time_sequence("frame", 1);
    rec.log("hallway/nodes", &GraphNodes::new(["area0"]))?;

    rec.set_time_sequence("frame", 2);
    rec.log("living/objects", &GraphNodes::new(["table"]))?;
    rec.log(
        "living/nodes",
        &GraphNodes::new(["area0", "area1", "area2"]),
    )?;
    rec.log(
        "living/edges",
        &GraphEdgesDirected::new([
            ("living/nodes", "area0", "area1"),
            ("living/nodes", "area0", "area2"),
            ("living/nodes", "area1", "area2"),
        ]),
    )?;

    rec.log(
        "doors/edges",
        &GraphEdgesDirected::new([
            (("kitchen/nodes", "area0"), ("hallway/nodes", "area0")),
            (("hallway/nodes", "area0"), ("living/nodes", "area2")),
        ]),
    )?;

    rec.log(
        "edges",
        &GraphEdgesUndirected::new([
            (("kitchen/nodes", "area0"), ("kitchen/objects", "sink")),
            (("kitchen/nodes", "area1"), ("kitchen/objects", "fridge")),
            (("living/nodes", "area1"), ("living/objects", "table")),
        ]),
    )?;
    Ok(())
}
