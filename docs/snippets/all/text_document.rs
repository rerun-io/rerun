//! Log a `TextDocument`

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_text_document").spawn()?;

    rec.log(
        "text_document",
        &rerun::TextDocument::new("Hello, TextDocument!"),
    )?;

    rec.log(
        "markdown",
        &rerun::TextDocument::new(
            r#"
# Hello Markdown!
[Click here to see the raw text](recording://markdown:Text).

Basic formatting:

| **Feature**       | **Alternative** |
| ----------------- | --------------- |
| Plain             |                 |
| *italics*         | _italics_       |
| **bold**          | __bold__        |
| ~~strikethrough~~ |                 |
| `inline code`     |                 |

----------------------------------

## Support
- [x] [Commonmark](https://commonmark.org/help/) support
- [x] GitHub-style strikethrough, tables, and checkboxes
- Basic syntax highlighting for:
  - [x] C and C++
  - [x] Python
  - [x] Rust
  - [ ] Other languages

## Links
You can link to [an entity](recording://markdown),
a [specific instance of an entity](recording://markdown[#0]),
or a [specific component](recording://markdown:Text).

Of course you can also have [normal https links](https://github.com/rerun-io/rerun), e.g. <https://rerun.io>.

## Image
![A random image](https://picsum.photos/640/480)
"#.trim(),
        )
        .with_media_type(rerun::MediaType::markdown()),
    )?;

    Ok(())
}
