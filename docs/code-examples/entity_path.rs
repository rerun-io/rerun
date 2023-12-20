//! Log a `TextDocument`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_text_document").spawn()?;

    rec.log(
        r"world/escaped\ string\!",
        &rerun::TextDocument::new("This entity path was escaped manually"),
    )?;
    rec.log(
        rerun::entity_path!["world", "unescaped string!"],
        &rerun::TextDocument::new("This entity path was provided as a list of unescaped strings"),
    )?;

    Ok(())
}
