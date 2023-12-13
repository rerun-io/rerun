//! Demonstrates how to log data to standard output with the Rerun SDK, and then visualize it
//! from standard input with the Rerun Viewer.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_stdio").stdout()?;

    let input = std::io::stdin()
        .lines()
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");

    rec.log("stdin", &rerun::TextDocument::new(input))?;

    Ok(())
}
