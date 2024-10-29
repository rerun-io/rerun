use rerun::{GraphEdges, GraphNodes};

use crate::Args;
use std::collections::HashMap;

pub fn run(args: &Args) -> anyhow::Result<()> {
    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_binary_tree")?;
    let s = 3.0; // scaling factor for the positions

    // Potentially unbalanced and not sorted binary tree. :nerd_face:.
    // :warning: The nodes have to be unique, which is why we use `5_0`…

    // (label, position)
    type NodeInfo = (&'static str, (f32, f32));

    let nodes: HashMap<&str, NodeInfo> = [
        ("1", ("1", (0.0 * s, 0.0 * s))),
        ("7", ("7", (-20.0 * s, 30.0 * s))),
        ("2", ("2", (-30.0 * s, 60.0 * s))),
        ("6", ("6", (-10.0 * s, 60.0 * s))),
        ("5_0", ("5", (-20.0 * s, 90.0 * s))),
        ("11", ("11", (0.0 * s, 90.0 * s))),
        ("9_0", ("9", (20.0 * s, 30.0 * s))),
        ("9_1", ("9", (30.0 * s, 60.0 * s))),
        ("5_1", ("5", (20.0 * s, 90.0 * s))),
    ]
    .into_iter()
    .collect();

    struct Level<'a> {
        nodes: &'a [&'a str],
        edges: &'a [(&'a str, &'a str)],
    }

    let levels: Vec<Level> = vec![
        Level {
            nodes: &["1"],
            edges: &[],
        },
        Level {
            nodes: &["1", "7", "9_0"],
            edges: &[("1", "7"), ("1", "9_0")],
        },
        Level {
            nodes: &["1", "7", "9_0", "2", "6", "9_1"],
            edges: &[
                ("1", "7"),
                ("1", "9_0"),
                ("7", "2"),
                ("7", "6"),
                ("9_0", "9_1"),
            ],
        },
        Level {
            nodes: &["1", "7", "9_0", "2", "6", "9_1", "5_0", "11", "5_1"],
            edges: &[
                ("1", "7"),
                ("1", "9_0"),
                ("7", "2"),
                ("7", "6"),
                ("9_0", "9_1"),
                ("6", "5_0"),
                ("6", "11"),
                ("9_1", "5_1"),
            ],
        },
    ];

    let mut t = 0;
    for level in levels {
        if !level.nodes.is_empty() {
            t += 1;
            rec.set_time_seconds("stable_time", t as f64);
            let _ = rec.log(
                "binary_tree",
                &GraphNodes::new(level.nodes.iter().copied())
                    .with_labels(level.nodes.iter().map(|n| nodes[n].0))
                    .with_positions(level.nodes.iter().map(|n| nodes[n].1)),
            );
        }

        if !level.edges.is_empty() {
            t += 1;
            rec.set_time_seconds("stable_time", t as f64);
            let _ = rec.log("binary_tree", &GraphEdges::new(level.edges));
        }
    }

    Ok(())
}
