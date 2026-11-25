//! Snapshot tests of a wide range of markdown syntax, with a focus on edge cases,
//! rendered with [`re_ui::UiExt::markdown_ui`].
//!
//! Each snapshot shows the markdown source on the left and the rendered result on the right.
//! The tests stick to `CommonMark`, plus tables.
//!
//! `markdown_test.rs` has the tests of how markdown fits into the surrounding UI.

use egui::Vec2;
use re_ui::UiExt as _;

/// The width of the source column and of the rendered column.
///
/// Narrow enough that long lines and wide tables have to wrap or scroll.
/// Markdown that overflows it makes the snapshot wider.
const WIDTH: f32 = 400.0;

/// A small image that the markdown can load.
const IMAGE_URI: &str = "bytes://rerun_logo.png";

fn snapshot_markdown(name: &str, markdown: &str) {
    let mut harness = re_ui::testing::new_harness(
        re_ui::testing::TestOptions::Gui,
        Vec2::new(2.0 * WIDTH + 50.0, 4000.0),
    )
    .build_ui(|ui| {
        ui.ctx().include_bytes(
            IMAGE_URI,
            include_bytes!("../../data/icons/rerun_logo.png").as_slice(),
        );
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(WIDTH);
                ui.add(egui::Label::new(egui::RichText::new(markdown).monospace()).wrap());
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(WIDTH);
                ui.markdown_ui(markdown);
            });
        });
    });
    harness.run();
    harness.fit_contents();
    harness.snapshot(format!("markdown_syntax/{name}"));
}

#[test]
fn headings() {
    snapshot_markdown(
        "headings",
        "\
# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6
Text right after a heading, with no blank line.

Setext heading 1
================

Setext heading 2
----------------

## Heading with `inline code`, *emphasis*, and a [link](https://rerun.io)

## A heading that is long enough that it has to wrap onto a second row

## A heading with a closing sequence ##

#NoSpace is not a heading.
",
    );
}

#[test]
fn emphasis() {
    snapshot_markdown(
        "emphasis",
        "\
*italic*, _italic_, **bold**, __bold__, ***bold italic***.

**Bold with *italic* inside**, and *italic with **bold** inside*.

Intraword underscores are not emphasis: snake_case_name and foo__bar__baz.
Intraword stars are: foo*bar*baz.
Underscores around a whole word are: __init__ is bold.

\\*Escaped stars\\*, \\`escaped backticks\\`, and a backslash: \\\\.

Entities: &copy; &amp; &lt;tag&gt; &#169;

A line ending in two spaces\x20\x20
is a hard break, and so is a backslash\\
at the end of a line. A single newline
is a soft break.
",
    );
}

#[test]
fn unordered_lists() {
    snapshot_markdown(
        "unordered_lists",
        "\
- dash
* star
+ plus

- Level 1
  - Level 2
    - Level 3
      - Level 4
  - Back to level 2
- An item that is long enough that it has to wrap onto a second row, which should line up with the text and not the bullet

A loose list:

- A loose item

  with a second paragraph

- Another loose item
",
    );
}

#[test]
fn ordered_lists() {
    snapshot_markdown(
        "ordered_lists",
        "\
Every item numbered `1.`, which should count up:

1. one
1. two
1. three

Every item numbered `1)`, which should also count up:

1) one
1) two
1) three

Indented, every item numbered `1)`:

  1) one
  1) two
     1) nested one
     1) nested two
  1) three

Starting at 7, and crossing into two digits:

7. seven
8. eight
9. nine
10. ten
11. eleven

Starting at zero:

0. zero
1. one

A paragraph line
2. that starts with a number other than 1 cannot interrupt a paragraph, so this is not a list.

Ordered inside unordered, and the other way around:

- Fruit
  1. Apple
  2. Banana
1. First
   - a bullet
   - another bullet
2. Second

A list of its own:

1. An item that is long enough that it has to wrap onto a second row, which should line up with the text and not the number
",
    );
}

#[test]
fn code_blocks() {
    snapshot_markdown(
        "code_blocks",
        "\
Fenced, with a language:

```rust
fn main() {
    println!(\"Hello, world!\");
}
```

Fenced, without a language:

```
plain text
```

Fenced with tildes:

~~~python
print(\"tildes\")
~~~

Indented with four spaces:

    indented code
    block

A line that is much wider than the view:

```
let very_long_line = some_function(with_many, arguments, that_do_not_fit, on_one_row, of_this_narrow_view);
```

An empty code block:

```
```

Inline code with backticks inside: `` `ticks` `` and `a\x20\x20b` (two spaces).

Inline code that is wider than the view: `let very_long_line = some_function(with_many, arguments, that_do_not_fit);`
",
    );
}

#[test]
fn code_in_containers() {
    snapshot_markdown(
        "code_in_containers",
        "\
1. A list item with a code block:

   ```rust
   let x = 42;
   ```

2. And a second item after it.

- A bullet with `inline code` that is long enough to wrap onto a second row of the list

> A quote with a code block:
>
> ```
> quoted code
> ```

> - A list in a quote
>   ```
>   code in a list in a quote
>   ```
",
    );
}

/// A code fence that is never closed, as in markdown that is still streaming in.
#[test]
fn unterminated_code_fence() {
    snapshot_markdown(
        "unterminated_code_fence",
        "\
Text before the code.

```rust
fn main() {
    println!(\"still streaming",
    );
}

#[test]
fn tables() {
    snapshot_markdown(
        "tables",
        "\
| Left | Center | Right |
|:-----|:------:|------:|
| a | b | c |
| longer cell | `code` | **bold** |
| [link](https://rerun.io) | *italic* | 1234.5 |
|  | empty cells |  |

A pipe in a cell, inside code and escaped `\\|`:

| Expression | Meaning |
|---|---|
| `a \\| b` | a or b |
| a \\| b | also a or b |

A cell with a lot of text that should wrap, or make the table scroll:

| Name | Description |
|---|---|
| wrap | This description is long enough that it does not fit on one row of this narrow view. |

A table right after a paragraph, without a blank line:
| a | b |
|---|---|
| 1 | 2 |
",
    );
}

/// A table cell cannot hold a code block, so long code in a cell is inline code.
#[test]
fn code_in_tables() {
    snapshot_markdown(
        "code_in_tables",
        "\
| Language | Example |
|---|---|
| Rust | `fn main() { run(); }` |
| Python | `print(1)` then `print(2)` |
| Long | `let result = some_function(with_many, arguments, that_do_not_fit);` |

A wide table:

| one | two | three | four | five | six | seven | eight | nine | ten |
|---|---|---|---|---|---|---|---|---|---|
| `1` | `2` | `3` | `4` | `5` | `6` | `7` | `8` | `9` | `10` |
",
    );
}

#[test]
fn blockquotes() {
    snapshot_markdown(
        "blockquotes",
        "\
> A simple quote.

> Level 1
>> Level 2
>>> Level 3

> ## A heading in a quote
>
> - a list
> - in a quote
>
> 1. numbered
> 2. in a quote

> A lazy continuation line,
which belongs to the quote without a `>`.

> A quote with a line that is long enough that it has to wrap onto a second row of the view.
",
    );
}

#[test]
fn links_and_images() {
    snapshot_markdown(
        "links_and_images",
        "\
An [inline link](https://rerun.io), a [link with a title](https://rerun.io \"Rerun\"),
and a [reference link][ref].

An autolink: <https://rerun.io>, and an email: <hello@rerun.io>.

A link with `code` inside: [`re_ui`](https://docs.rs/re_ui).

A link that is too long for one row: <https://github.com/rerun-io/rerun/blob/main/crates/ui_tools/re_ui/src/ui_ext.rs>

An image: ![The Rerun logo](bytes://rerun_logo.png)

An image that cannot load: ![alt text](does-not-exist.png)

[ref]: https://rerun.io
",
    );
}

/// Thematic breaks, unicode, and an unbreakable word.
#[test]
fn misc() {
    snapshot_markdown(
        "misc",
        "\
Thematic breaks:

---

***

___

Unicode: café, naïve, Ελληνικά, Кириллица, and typographic “quotes” – dashes…

AVeryLongWordWithoutAnySpacesThatCannotWrapAtAWordBoundaryAndMustBreakSomewhere
",
    );
}

#[test]
fn whitespace_only() {
    snapshot_markdown("whitespace_only", "\n\n   \n");
}
