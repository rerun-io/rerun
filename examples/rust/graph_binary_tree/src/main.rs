//! Shows how to draw a graph that varies over time.
//! Please not that this example makes use of fixed positions.
//!
//! Usage:
//! ```
//!  cargo run -p graph_binary_tree -- --connect
//! ```

use rerun::{external::re_log, GraphEdges, GraphNodes};
use std::collections::HashMap;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
pub struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}


fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_binary_tree")?;

    let s = 3.0; // scaling factor for the positions

    // Potentially unbalanced and not sorted binary tree. :nerd_face:.
    // :warning: The nodes have to be unique, which is why we use `5_0`…

    // (label, position)
    type NodeInfo = (&'static str, (f32, f32));

    struct Level<'a> {
        nodes: &'a [&'a str],
        edges: &'a [(&'a str, &'a str)],
    }

    let nodes_unsorted: HashMap<&str, NodeInfo> = [
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

    let levels_unsorted: Vec<Level> = vec![
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

    let nodes_sorted: HashMap<&str, NodeInfo> = [
        ("6", ("6", (0.0 * s, 0.0 * s))),
        ("5_0", ("5", (-20.0 * s, 30.0 * s))),
        ("9_0", ("9", (20.0 * s, 30.0 * s))),
    ]
    .into_iter()
    .collect();

    let levels_sorted: Vec<Level> = vec![
        Level {
            nodes: &["6"],
            edges: &[],
        },
        Level {
            nodes: &["6", "5_0", "9_0"],
            edges: &[("6", "5_0"), ("6", "9_0"), ("1", "6"), ("1", "42")],
        },
    ];

    let mut t = 0;
    for level in levels_unsorted {
        if !level.nodes.is_empty() {
            t += 1;
            rec.set_time_seconds("stable_time", t as f64);
            let _ = rec.log(
                "unsorted",
                &GraphNodes::new(level.nodes.iter().copied())
                    .with_labels(level.nodes.iter().map(|n| nodes_unsorted[n].0))
                    .with_positions(level.nodes.iter().map(|n| nodes_unsorted[n].1)),
            );
        }

        if !level.edges.is_empty() {
            t += 1;
            rec.set_time_seconds("stable_time", t as f64);
            let _ = rec.log("unsorted", &GraphEdges::new(level.edges));
        }
    }

    let entity_offset_x = 200.0;

    for level in levels_sorted {
        if !level.nodes.is_empty() {
            t += 1;
            rec.set_time_seconds("stable_time", t as f64);
            let _ = rec.log(
                "sorted",
                &GraphNodes::new(level.nodes.iter().copied())
                    .with_labels(level.nodes.iter().map(|n| nodes_sorted[n].0))
                    .with_positions(level.nodes.iter().map(|n| {
                        let (x, y) = nodes_sorted[n].1;
                        [x + entity_offset_x, y]
                    })),
            );
        }

        if !level.edges.is_empty() {
            t += 1;
            rec.set_time_seconds("stable_time", t as f64);
            let _ = rec.log(
                "sorted",
                &GraphEdges::new(level.edges).with_directed_edges(),
            );
        }
    }

    Ok(())
}
