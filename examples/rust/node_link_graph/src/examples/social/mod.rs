use itertools::Itertools;
use rerun::{
    components::{self},
    datatypes,
    external::log,
    GraphEdgesUndirected, GraphNodes,
};

use crate::Args;
use std::{
    collections::HashSet,
    io::{BufRead, BufReader},
};

struct Interaction {
    timestamp: u32,
    person_a: datatypes::GraphNodeId,
    person_b: datatypes::GraphNodeId,
}

fn parse_data_file() -> anyhow::Result<Vec<Interaction>> {
    let contents = include_str!("tij_SFHH.dat_");
    let cursor = std::io::Cursor::new(contents.as_bytes());
    let reader = BufReader::new(cursor);

    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<String> = line
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();

        let t = parts[0].as_str();
        let i = parts[1].as_str();
        let j = parts[2].as_str();

        entries.push(Interaction {
            timestamp: t.parse::<u32>()?,
            person_a: i.into(),
            person_b: j.into(),
        });
    }

    Ok(entries)
}

pub fn run(args: &Args) -> anyhow::Result<()> {
    let (rec, _serve_guard) = args.rerun.init("rerun_example_graph_social")?;

    // rec.set_time_sequence("frame", 0);
    let entries = parse_data_file()?;

    for (timestamp, chunk) in &entries.into_iter().chunk_by(|t| t.timestamp) {

        let interactions = chunk.collect::<Vec<_>>();

        let mut nodes = HashSet::new();
        for i in interactions.iter() {
            nodes.insert(i.person_a.clone());
            nodes.insert(i.person_b.clone());
        }

        if nodes.is_empty() {
            continue;
        }

        log::info!("Logging nodes for timestamp `{timestamp}`: {:?}", nodes);

        rec.set_time_sequence("frame", timestamp);

        rec.log(
            "/persons",
            &GraphNodes::new(nodes.iter().map(|n| {
                components::GraphNodeId::from(datatypes::GraphNodeId(n.to_string().into()))
            })),
        )?;

        rec.log(
            "/interactions",
            &GraphEdgesUndirected::new(interactions.into_iter().map(|i| ("/persons", i.person_a, i.person_b))),
        )?;
    }
    Ok(())
}
