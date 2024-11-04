//! Log a simple undirected graph.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_graph_undirected").spawn()?;

    rec.log(
        "simple",
        &rerun::GraphNodes::new(["a", "b", "c"])
            .with_positions([(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)])
            .with_labels(["A", "B", "C"]),
    )?;
    // Note: We log to the same entity here.
    rec.log(
        "simple",
        &rerun::GraphEdges::new([("a", "b"), ("b", "c"), ("c", "a")]).with_undirected_edges(), // Optional: graphs are undirected by default.
    )?;

    Ok(())
}
