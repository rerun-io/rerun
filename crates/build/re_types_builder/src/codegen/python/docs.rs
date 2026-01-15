//! Documentation generation helpers for Python codegen.

use itertools::Itertools as _;

use super::typing::quote_field_type_from_field;
use crate::codegen::common::{Example, ExampleInfo, collect_snippets_for_api_docs};
use crate::codegen::Target;
use crate::objects::State;
use crate::{Docs, Object, ObjectField, Objects, Reporter};

pub fn quote_examples(examples: Vec<Example<'_>>, lines: &mut Vec<String>) {
    let mut examples = examples.into_iter().peekable();
    while let Some(example) = examples.next() {
        let ExampleInfo {
            path,
            name,
            title,
            image,
            ..
        } = &example.base;

        for line in &example.lines {
            assert!(
                !line.contains("```"),
                "Example {path:?} contains ``` in it, so we can't embed it in the Python API docs."
            );
            assert!(
                !line.contains("\"\"\""),
                "Example {path:?} contains \"\"\" in it, so we can't embed it in the Python API docs."
            );
        }

        if let Some(title) = title {
            lines.push(format!("### {title}:"));
        } else {
            lines.push(format!("### `{name}`:"));
        }
        lines.push("```python".into());
        lines.extend(example.lines.into_iter());
        lines.push("```".into());
        if let Some(image) = &image {
            lines.extend(
                // Don't let the images take up too much space on the page.
                image.image_stack().center().width(640).finish(),
            );
        }
        if examples.peek().is_some() {
            // blank line between examples
            lines.push(String::new());
        }
    }
}

/// Ends with double newlines, unless empty.
pub fn quote_obj_docs(reporter: &Reporter, objects: &Objects, obj: &Object) -> String {
    let mut lines = lines_from_docs(reporter, objects, &obj.docs, &obj.state);

    if let Some(first_line) = lines.first_mut() {
        // Prefix with object kind:
        *first_line = format!("**{}**: {}", obj.kind.singular_name(), first_line);
    }

    quote_doc_lines(lines)
}

pub fn lines_from_docs(
    reporter: &Reporter,
    objects: &Objects,
    docs: &Docs,
    state: &State,
) -> Vec<String> {
    let mut lines = docs.lines_for(reporter, objects, Target::Python);

    if let Some(docline_summary) = state.docline_summary() {
        lines.push(String::new());
        lines.push(docline_summary);
    }

    let examples = collect_snippets_for_api_docs(docs, "py", true).unwrap_or_else(|err| {
        reporter.error_any(err);
        vec![]
    });

    if !examples.is_empty() {
        lines.push(String::new());
        let (section_title, divider) = if examples.len() == 1 {
            ("Example", "-------")
        } else {
            ("Examples", "--------")
        };
        lines.push(section_title.into());
        lines.push(divider.into());
        quote_examples(examples, &mut lines);
    }

    lines
}

/// Ends with double newlines, unless empty.
pub fn quote_doc_lines(lines: Vec<String>) -> String {
    if lines.is_empty() {
        return String::new();
    }

    for line in &lines {
        assert!(
            !line.contains("\"\"\""),
            "Cannot put triple quotes in Python docstrings"
        );
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise…
    let lines: Vec<String> = lines
        .into_iter()
        .filter(|line| !line.starts_with(r#"""""#))
        .collect();

    if lines.len() == 1 {
        // single-line
        let line = &lines[0];
        format!("\"\"\"{line}\"\"\"\n\n") // NOLINT
    } else {
        // multi-line
        format!("\"\"\"\n{}\n\"\"\"\n\n", lines.join("\n"))
    }
}

pub fn quote_doc_from_fields(
    reporter: &Reporter,
    objects: &Objects,
    fields: &Vec<ObjectField>,
) -> String {
    let mut lines = vec!["Must be one of:".to_owned(), String::new()];

    for field in fields {
        let mut content = field.docs.lines_for(reporter, objects, Target::Python);
        for line in &mut content {
            if line.starts_with(char::is_whitespace) {
                line.remove(0);
            }
        }

        let examples = collect_snippets_for_api_docs(&field.docs, "py", true).unwrap();
        if !examples.is_empty() {
            content.push(String::new()); // blank line between docs and examples
            quote_examples(examples, &mut lines);
        }
        lines.push(format!(
            "* {} ({}):",
            field.name,
            quote_field_type_from_field(objects, field, false).0
        ));
        lines.extend(content.into_iter().map(|line| format!("    {line}")));
        lines.push(String::new());
    }

    if lines.is_empty() {
        return String::new();
    } else {
        // remove last empty line
        lines.pop();
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise…
    let doc = lines
        .into_iter()
        .filter(|line| !line.starts_with(r#"""""#))
        .collect_vec()
        .join("\n");

    format!("\"\"\"\n{doc}\n\"\"\"\n\n")
}

pub fn quote_union_kind_from_fields(
    reporter: &Reporter,
    objects: &Objects,
    fields: &Vec<ObjectField>,
) -> String {
    let mut lines = vec!["Possible values:".to_owned(), String::new()];

    for field in fields {
        let mut content = field.docs.lines_for(reporter, objects, Target::Python);
        for line in &mut content {
            if line.starts_with(char::is_whitespace) {
                line.remove(0);
            }
        }
        lines.push(format!("* {:?}:", field.snake_case_name()));
        lines.extend(content.into_iter().map(|line| format!("    {line}")));
        lines.push(String::new());
    }

    if lines.is_empty() {
        return String::new();
    } else {
        // remove last empty line
        lines.pop();
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise…
    let doc = lines
        .into_iter()
        .filter(|line| !line.starts_with(r#"""""#))
        .collect_vec()
        .join("\n");

    format!("\"\"\"\n{doc}\n\"\"\"\n\n")
}
