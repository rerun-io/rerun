//! Query and display the first 10 rows of a recording in a dataframe view.
//!
//! The blueprint is being loaded from an existing blueprint recording file.

// cargo r -p snippets -- dataframe_view_query_external /tmp/dna.rrd /tmp/dna.rbl

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();

    let path_to_rrd = &args[1];
    let path_to_rbl = &args[2];

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_dataframe_view_query_external")
        .spawn()?;

    rec.log_file_from_path(path_to_rrd, None /* prefix */, false /* static */)?;
    rec.log_file_from_path(path_to_rbl, None /* prefix */, false /* static */)?;

    Ok(())
}
