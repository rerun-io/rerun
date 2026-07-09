//! Use a blueprint to show a text document.

use rerun::blueprint::{Blueprint, TextDocumentView};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        TextDocumentView::new("Markdown example").with_origin("markdown"),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_text_document")
        .with_blueprint(blueprint)
        .spawn()?;

    rec.log(
        "markdown",
        &rerun::TextDocument::from_markdown(
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
"#
            .trim(),
        ),
    )?;

    Ok(())
}
