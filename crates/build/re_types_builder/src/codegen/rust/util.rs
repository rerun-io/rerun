//! Implements the Rust codegen pass.

use camino::Utf8Path;
use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::Target;
use crate::codegen::common::{ExampleInfo, collect_snippets_for_api_docs};
use crate::objects::State;
use crate::{ATTR_RUST_TUPLE_STRUCT, Docs, Object, ObjectKind, Objects, Reporter};

// ---

/// We put this magic prefix in `#[doc = "` attribute,
/// and then in the last step of codegen we transform that `#[doc`
/// not into a `///`, but to a normal `//` comment.
pub const SIMPLE_COMMENT_PREFIX: &str = "!COMMENT!";

/// Put a normal `// comment` in the code.
pub fn quote_comment(comment: &str) -> TokenStream {
    let lines = comment.lines().map(|line| {
        let line = format!("{SIMPLE_COMMENT_PREFIX}{line}");
        quote!(
            #[doc = #line]
        )
    });
    quote!(#(#lines)*)
}

pub fn is_tuple_struct_from_obj(obj: &Object) -> bool {
    if !obj.is_struct() {
        return false;
    }

    let is_tuple_struct = obj.kind == ObjectKind::Component
        || obj.try_get_attr::<String>(ATTR_RUST_TUPLE_STRUCT).is_some();

    if is_tuple_struct {
        assert!(
            obj.fields.len() == 1,
            "`{ATTR_RUST_TUPLE_STRUCT}` is only supported for objects with a single field, but {} has {}",
            obj.fqname,
            obj.fields.len(),
        );
    }

    is_tuple_struct
}

pub fn string_from_quoted(
    reporter: &Reporter,
    acc: &TokenStream,
    target_file: &Utf8Path,
) -> String {
    re_tracing::profile_function!();

    // We format using `prettyplease` because there are situations with
    // very long lines that `cargo fmt` fails on.
    // See https://github.com/dtolnay/prettyplease for more info.

    let string = match syn::parse_file(&acc.to_string()) {
        Ok(parsed) => prettyplease::unparse(&parsed),
        Err(err) => {
            reporter.error_file(
                target_file,
                format!("Generated Rust code did not parse: {err}"),
            );
            acc.to_string()
        }
    };

    // `prettyplease` formats docstrings weirdly, like so:
    //
    // struct Foo {
    //     ///No leading space
    //     bar: i32,
    //     ///And no empty space before the first line
    //     ///of the doscstring
    //     baz: f64,
    // }
    //
    // We fix that here,
    // while also adding blank lines before functions and `impl` blocks.

    let mut output = String::default();
    let mut is_in_docstring = false;
    let mut prev_line_was_docstring = false;
    let mut prev_line_was_attr = false;

    for line in string.split('\n') {
        if let Some(slashes) = line.find("///") {
            let leading_spaces = &line[..slashes];
            if leading_spaces.trim().is_empty() {
                // This is a docstring

                if !is_in_docstring {
                    output.push('\n');
                }
                let comment = &line[slashes + 3..];
                output.push_str(leading_spaces);

                // TODO(emilk): why do we need to do the `SIMPLE_COMMENT_PREFIX` both here and in `fn replace_doc_attrb_with_doc_comment`?
                if let Some(comment) = comment.strip_prefix(SIMPLE_COMMENT_PREFIX) {
                    output.push_str("//");
                    if !comment.starts_with(char::is_whitespace) {
                        output.push(' ');
                    }
                    output.push_str(comment);
                } else {
                    output.push_str("///");
                    if !comment.starts_with(char::is_whitespace) {
                        output.push(' ');
                    }
                    output.push_str(comment);
                }
                output.push('\n');

                prev_line_was_attr = false;
                is_in_docstring = true;
                prev_line_was_docstring = true;

                continue;
            }
        }

        is_in_docstring = false;

        // Insert some extra newlines before functions and `impl` blocks:
        let trimmed = line.trim_start();

        let line_is_attr = trimmed.starts_with("#[allow(")
            || trimmed.starts_with("#[expect(")
            || trimmed.starts_with("#[inline]")
            || trimmed.starts_with("#[doc(hidden)]")
            || trimmed.starts_with("#[rustfmt::skip]")
            || trimmed.starts_with("#[derive");

        if line_is_attr && (!prev_line_was_attr && !prev_line_was_docstring) {
            output.push('\n');
        }

        if !prev_line_was_attr
            && (trimmed.starts_with("const ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("impl<")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("static ")
                || trimmed.starts_with("::re_types_core::macros"))
        {
            output.push('\n');
        }

        output.push_str(line);
        output.push('\n');
        prev_line_was_attr = line_is_attr;
        prev_line_was_docstring = false;
    }

    output
}

pub fn append_tokens(
    reporter: &Reporter,
    mut code: String,
    quoted_obj: &TokenStream,
    target_file: &Utf8Path,
) -> String {
    code.push_str(&string_from_quoted(reporter, quoted_obj, target_file));
    code.push('\n');
    replace_doc_attrb_with_doc_comment(&code)
}

/// Replace `#[doc = "…"]` attributes with `/// …` doc comments,
/// while also removing trailing whitespace.
fn replace_doc_attrb_with_doc_comment(code: &str) -> String {
    // This is difficult to do with regex, because the patterns with newlines overlap.

    let start_pattern = "# [doc = \"";
    let end_pattern = "\"]\n"; // assures there is no escaped quote followed by a bracket

    let problematic = r#"\"]\n"#;
    assert!(
        !code.contains(problematic),
        "The codegen cannot handle the string {problematic} yet"
    );

    let mut new_code = String::new();

    let mut i = 0;
    while i < code.len() {
        if let Some(off) = code[i..].find(start_pattern) {
            let doc_start = i + off;
            let content_start = doc_start + start_pattern.len();
            if let Some(off) = code[content_start..].find(end_pattern) {
                let content_end = content_start + off;
                let content = &code[content_start..content_end];
                let mut unescped_content = unescape_string(content);

                new_code.push_str(&code[i..doc_start]);

                // TODO(emilk): why do we need to do the `SIMPLE_COMMENT_PREFIX` both here and in `fn string_from_quoted`?
                if let Some(rest) = unescped_content.strip_prefix(SIMPLE_COMMENT_PREFIX) {
                    // This is a normal comment
                    new_code.push_str("//");
                    unescped_content = rest.to_owned();
                } else {
                    // This is a docstring
                    new_code.push_str("///");
                }

                if !content.starts_with(char::is_whitespace) {
                    new_code.push(' ');
                }
                new_code.push_str(&unescped_content);
                new_code.push('\n');

                i = content_end + end_pattern.len();
                // Skip trailing whitespace (extra newlines)
                while matches!(code.as_bytes().get(i), Some(b'\n' | b' ')) {
                    i += 1;
                }
                continue;
            }
        }

        // No more doc attributes found
        new_code.push_str(&code[i..]);
        break;
    }
    new_code
}

#[test]
fn test_doc_attr_unfolding() {
    // Normal case with unescaping of quotes:
    assert_eq!(
        replace_doc_attrb_with_doc_comment(
            r#"
# [doc = "Hello, \"world\"!"]
pub fn foo () {}
        "#
        ),
        r#"
/// Hello, "world"!
pub fn foo () {}
        "#
    );

    // Spacial case for when it contains a `SIMPLE_COMMENT_PREFIX`:
    assert_eq!(
        replace_doc_attrb_with_doc_comment(&format!(
            r#"
# [doc = "{SIMPLE_COMMENT_PREFIX}Just a \"comment\"!"]
const FOO: u32 = 42;
        "#
        )),
        r#"
// Just a "comment"!
const FOO: u32 = 42;
        "#
    );
}

fn unescape_string(input: &str) -> String {
    let mut output = String::new();
    unescape_string_into(input, &mut output);
    output
}

fn unescape_string_into(input: &str, output: &mut String) {
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            let c = chars.next().expect("Trailing backslash");
            match c {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '\\' => output.push('\\'),
                '"' => output.push('"'),
                '\'' => output.push('\''),
                _ => panic!("Unknown escape sequence: \\{c}"),
            }
        } else {
            output.push(c);
        }
    }
}

// ----------------------------------------------------------------------------

pub fn doc_as_lines(
    reporter: &Reporter,
    objects: &Objects,
    virtpath: &str,
    fqname: &str,
    state: &State,
    docs: &Docs,
    target: Target,
) -> Vec<String> {
    let mut lines = docs.lines_for(reporter, objects, target);

    if let Some(docline_summary) = state.docline_summary() {
        lines.push(String::new());
        lines.push(docline_summary);
    }

    let examples = if !fqname.starts_with("rerun.blueprint.views") {
        collect_snippets_for_api_docs(docs, "rs", true)
            .map_err(|err| reporter.error(virtpath, fqname, err))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    if !examples.is_empty() {
        lines.push(Default::default());
        let section_title = if examples.len() == 1 {
            "Example"
        } else {
            "Examples"
        };
        lines.push(format!("## {section_title}"));
        lines.push(Default::default());
        let mut examples = examples.into_iter().peekable();
        while let Some(example) = examples.next() {
            let ExampleInfo {
                path,
                name,
                title,
                image,
                ..
            } = &example.base;

            if example.lines.iter().any(|line| line.contains("```")) {
                reporter.error(
                    virtpath,
                    fqname,
                    format!("Example {path:?} contains ``` in it, so we can't embed it in the Rust API docs."),
                );
                continue;
            }

            if let Some(title) = title {
                lines.push(format!("### {title}"));
            } else {
                lines.push(format!("### `{name}`:"));
            }

            lines.push("```ignore".into());
            lines.extend(example.lines.into_iter());
            lines.push("```".into());

            if let Some(image) = &image {
                // Don't let the images take up too much space on the page.
                lines.extend(image.image_stack().center().width(640).finish());
            }
            if examples.peek().is_some() {
                // blank line between examples
                lines.push(Default::default());
            }
        }
    }

    if let Some(second_line) = lines.get(1)
        && !second_line.is_empty()
    {
        reporter.warn(
            virtpath,
            fqname,
            format!("Second line of documentation should be an empty line; found {second_line:?}"),
        );
    }

    lines
}

pub fn quote_doc_line(line: &str) -> TokenStream {
    let line = format!(" {line}"); // add space between `///` and comment
    quote!(# [doc = #line])
}

pub fn quote_doc_lines(lines: &[String]) -> TokenStream {
    struct DocCommentTokenizer<'a>(&'a [String]);

    impl quote::ToTokens for DocCommentTokenizer<'_> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.extend(self.0.iter().map(|line| {
                let line = format!(" {line}"); // add space between `///` and comment
                quote!(# [doc = #line])
            }));
        }
    }

    let lines = DocCommentTokenizer(lines);
    quote!(#lines)
}
