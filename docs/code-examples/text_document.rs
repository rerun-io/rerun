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
            r#"
# Hello Markdown!
[Click here to see the raw text](recording://markdown.Text).

Basic formatting:

| **Feature**       | **Alternative** |
| ----------------- | --------------- |
| Plain             |                 |
| *italics*         | _italics_       |
| **bold**          | __bold__        |
| ~~strikethrough~~ |                 |
| `inline code`     |                 |

----------------------------------

Some code:
```rs
fn main() {
    println!("Hello, world!");
}
```

## Support
- [x] [Commonmark](https://commonmark.org/help/) support
- [x] GitHub-style strikethrough, tables, and checkboxes
- [ ] Syntax highlighting

## Links
You can link to [an entity](recording://markdown),
a [specific instance of an entity](recording://markdown[#0]),
or a [specific component](recording://markdown.Text).

Of course you can also have [normal https links](https://github.com/rerun-io/rerun), e.g. <https://rerun.io>.

## Image
![A random image](https://picsum.photos/640/480)
"#.trim(),
        )
        .with_media_type(MediaType::markdown()),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
