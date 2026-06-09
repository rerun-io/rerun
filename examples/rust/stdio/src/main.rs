//! Demonstrates how to log data to standard output with the Rerun SDK, and then visualize it
//! from standard input with the Rerun Viewer.
//!
//! Usage:
//! ```text
//! echo 'hello from stdin!' | cargo run | rerun -
//! ```

use itertools::Itertools as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_stdio").stdout()?;

    let lines: Vec<String> = std::io::stdin().lines().try_collect()?;
    let input = lines.join("\n");

    rec.log("stdin", &rerun::TextDocument::new(input))?;

    Ok(())
}
