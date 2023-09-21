// Log a `TextDocument`

#include <rerun.hpp>

#include <cmath>

namespace rr = rerun;
namespace rrd = rr::datatypes;

int main() {
    auto rr_stream = rr::RecordingStream("rerun_example_text_document");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    rr_stream.log("text_document", rr::archetypes::TextDocument("Hello, TextDocument!"));

    rr_stream.log(
        "markdown",
        rr::archetypes::TextDocument(R"#(# Hello Markdown!
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
![A random image](https://picsum.photos/640/480))#")
            .with_media_type(rr::components::MediaType::markdown())
    );
}
