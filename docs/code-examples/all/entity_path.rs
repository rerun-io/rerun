//! Example of different ways of constructing an entity path.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_entity_path").spawn()?;

    rec.log(
        r"world/42/escaped\ string\!",
        &rerun::TextDocument::new("This entity path was escaped manually"),
    )?;
    rec.log(
        rerun::entity_path!["world", 42, "unescaped string!"],
        &rerun::TextDocument::new("This entity path was provided as a list of unescaped strings"),
    )?;

    Ok(())
}
