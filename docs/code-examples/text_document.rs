//! Log a `TextDocument`

use rerun::{
    archetypes::TextDocument, external::re_types::components::MediaType, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_text_document").memory()?;

    rec.log("text_document", &TextDocument::new("Hello, TextDocument!"))?;
    rec.log(
        "markdown",
        &TextDocument::new(
            "# Hello\n\
             Markdown with `code`!\n\
             \n\
             A random image:\n\
             \n\
             ![A random image](https://picsum.photos/640/480)",
        )
        .with_media_type(MediaType::markdown()),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
