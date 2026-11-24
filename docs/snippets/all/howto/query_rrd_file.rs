//! Load an `.rrd` file and query it with the dataframe API.

// region: query
use rerun::ChunkStoreConfig;
use rerun::dataframe::{
    QueryEngine, QueryExpression, SparseFillStrategy, TimelineName,
};

fn query_rrd(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let engines =
        QueryEngine::from_rrd_filepath(&ChunkStoreConfig::DEFAULT, path)?;

    for (store_id, engine) in engines {
        if !store_id.is_recording() {
            continue;
        }

        let query = QueryExpression {
            filtered_index: Some(TimelineName::from("frame_nr")),
            sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
            ..Default::default()
        };

        let mut query_handle = engine.query(query);
        for batch in query_handle.batch_iter() {
            println!("{batch:?}"); // An Arrow `RecordBatch`
        }
    }

    Ok(())
}
// endregion: query

fn write_recording(
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_query_rrd_file")
            .save(path)?;
    for frame_nr in 0..10 {
        rec.set_time_sequence("frame_nr", frame_nr);
        rec.log(
            "/robot/gripper/force",
            &rerun::Scalars::single(frame_nr as f64),
        )?;
    }
    rec.flush_blocking()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[expect(
        clippy::disallowed_methods,
        reason = "the snippets crate has no tempdir dependency"
    )]
    let path = std::env::temp_dir().join(format!(
        "rerun_example_query_rrd_file_{}.rrd",
        std::process::id()
    ));
    write_recording(&path)?;
    let result = query_rrd(&path);
    std::fs::remove_file(&path).ok();
    result
}
