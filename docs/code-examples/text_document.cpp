// Log a `TextDocument`

#include <rerun.hpp>

#include <cmath>

namespace rrd = rerun::datatypes;

int main() {
    auto rec = rerun::RecordingStream("rerun_example_text_document");
    rec.spawn().throw_on_failure();

    rec.log("text_document", rerun::archetypes::TextDocument("Hello, TextDocument!"));

    rec.log(
        "markdown",
        rerun::archetypes::TextDocument(R"#(# Hello Markdown!
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
- Basic syntax highlighting for:
  - [x] C and C++
  - [x] Python
  - [x] Rust
  - [ ] Other languages

## Links
You can link to [an entity](recording://markdown),
a [specific instance of an entity](recording://markdown[#0]),
or a [specific component](recording://markdown.Text).

Of course you can also have [normal https links](https://github.com/rerun-io/rerun), e.g. <https://rerun.io>.

## Image
![A random image](https://picsum.photos/640/480))#")
            .with_media_type(rerun::components::MediaType::markdown())
    );
}
