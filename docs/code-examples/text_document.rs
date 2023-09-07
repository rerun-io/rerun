//! Log a `TextDocument`

use rerun::{archetypes::TextDocument, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_text_document").memory()?;

    rec.log("text_document", &TextDocument::new("Hello, TextDocument!"))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
