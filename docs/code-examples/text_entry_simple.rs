//! Log some text entries
use rerun::components::TextEntry;
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_text_entry").memory()?;

    MsgSender::new("logs")
        .with_component(&[TextEntry::new(
            "this entry as loglevel TRACE",
            Some("TRACE".into()),
        )])?
        .send(&rec)?;

    MsgSender::new("logs")
        .with_component(&[TextEntry::new(
            "this other entry as loglevel INFO",
            Some("INFO".into()),
        )])?
        .send(&rec)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
