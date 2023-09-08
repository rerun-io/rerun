//! Log some text entries
use rerun::components::TextEntry;
use rerun::RecordingStreamBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_text_entry").memory()?;

    // TODO(#2793): TextLog archetype
    rec.log_component_batches(
        "logs",
        false,
        1,
        [&TextEntry::new("this entry has loglevel TRACE", Some("TRACE".into())) as _],
    )?;
    rec.log_component_batches(
        "logs",
        false,
        1,
        [&TextEntry::new("this other entry has loglevel INFO", Some("INFO".into())) as _],
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
