#!/usr/bin/env python3
"""Log a `TextDocument`."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_text_document", spawn=True)

rr2.log("text_document", rr2.TextDocument(body="Hello, TextDocument!"))

rr2.log(
    "markdown",
    rr2.TextDocument(
        body="""
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
![A random image](https://picsum.photos/640/480)
""".strip(),
        media_type="text/markdown",
    ),
)
