//! Load an MCAP file using the Rust SDK.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path_to_mcap = std::env::args().nth(2).ok_or("Missing MCAP file")?;

    // Initialize the SDK and give our recording a unique name
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_load_mcap").spawn()?;

    // Load the MCAP file
    rec.log_file_from_path(path_to_mcap, None, false)?;

    Ok(())
}
