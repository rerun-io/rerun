//! Log a simple directed graph.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_graph_directed").spawn()?;

    rec.log(
        "simple",
        &[
            &rerun::GraphNodes::new(["a", "b", "c"])
                .with_positions([(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)])
                .with_labels(["A", "B", "C"]) as &dyn rerun::AsComponents,
            &rerun::GraphEdges::new([("a", "b"), ("b", "c"), ("c", "a")]).with_directed_edges(),
        ],
    )?;

    Ok(())
}
