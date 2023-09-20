//! Helpers common to all codegen passes.

use std::collections::BTreeSet;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use itertools::Itertools as _;

use crate::Docs;

fn is_blank<T: AsRef<str>>(line: T) -> bool {
    line.as_ref().chars().all(char::is_whitespace)
}

/// Retrieves the global and tagged documentation from a [`Docs`] object.
pub fn get_documentation(docs: &Docs, tags: &[&str]) -> Vec<String> {
    let mut lines = docs.doc.clone();

    for tag in tags {
        lines.extend(
            docs.tagged_docs
                .get(*tag)
                .unwrap_or(&Vec::new())
                .iter()
                .cloned(),
        );
    }

    // NOTE: remove duplicated blank lines.
    lines.dedup();

    // NOTE: remove trailing blank lines.
    while let Some(line) = lines.last() {
        if line.is_empty() {
            lines.pop();
        } else {
            break;
        }
    }

    lines
}

struct Example<'a> {
    name: &'a str,
    title: Option<&'a str>,
}

impl<'a> Example<'a> {
    fn parse(tag_content: &'a str) -> Self {
        let tag_content = tag_content.trim();
        let (name, title) = tag_content
            .split_once(' ')
            .map_or((tag_content, None), |(a, b)| (a, Some(b)));
        let title = title
            .and_then(|title| title.strip_prefix('"'))
            .and_then(|title| title.strip_suffix('"'));

        Example { name, title }
    }
}

pub fn get_examples(
    docs: &Docs,
    extension: &str,
    prefix: &[&str],
    suffix: &[&str],
) -> anyhow::Result<Vec<String>> {
    let mut lines = Vec::new();

    if let Some(examples) = docs.tagged_docs.get("example") {
        let base_path = crate::rerun_workspace_path().join("docs/code-examples");

        let mut examples = examples.iter().map(String::as_str).peekable();
        while let Some(Example { name, title }) = examples.next().map(Example::parse) {
            let path = base_path.join(format!("{name}.{extension}"));
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("couldn't open code example {path:?}"))?;
            let mut contents = contents.split('\n').collect_vec();
            // trim trailing blank lines
            while contents.last().is_some_and(is_blank) {
                contents.pop();
            }

            // prepend title if available
            lines.extend(title.into_iter().map(|title| format!("{title}:")));
            // surround content in prefix + suffix lines
            lines.extend(prefix.iter().copied().map(String::from));
            lines.extend(contents.into_iter().map(String::from));
            lines.extend(suffix.iter().copied().map(String::from));

            if examples.peek().is_some() {
                // blank line between examples
                lines.push(String::new());
            }
        }
    }

    Ok(lines)
}

pub trait StringExt {
    fn push_text(&mut self, text: impl AsRef<str>, linefeeds: usize, indent: usize) -> &mut Self;
    fn push_unindented_text(&mut self, text: impl AsRef<str>, linefeeds: usize) -> &mut Self;
}

impl StringExt for String {
    fn push_text(&mut self, text: impl AsRef<str>, linefeeds: usize, indent: usize) -> &mut Self {
        self.push_str(&indent::indent_all_by(indent, text.as_ref()));
        self.push_str(&vec!["\n"; linefeeds].join(""));
        self
    }

    fn push_unindented_text(&mut self, text: impl AsRef<str>, linefeeds: usize) -> &mut Self {
        self.push_str(&unindent::unindent(text.as_ref()));
        self.push_str(&vec!["\n"; linefeeds].join(""));
        self
    }
}

/// Remove all files in the given folder that are not in the given set.
pub fn remove_old_files_from_folder(folder_path: Utf8PathBuf, filepaths: &BTreeSet<Utf8PathBuf>) {
    re_tracing::profile_function!();
    re_log::debug!("Checking for old files in {folder_path}");
    for entry in std::fs::read_dir(folder_path).unwrap().flatten() {
        if entry.file_type().unwrap().is_dir() {
            continue;
        }
        let filepath = Utf8PathBuf::try_from(entry.path()).unwrap();

        if let Some(stem) = filepath.as_str().strip_suffix("_ext.rs") {
            let generated_path = Utf8PathBuf::try_from(format!("{stem}.rs")).unwrap();
            assert!(
                generated_path.exists(),
                "Found orphaned {filepath} with no matching {generated_path}"
            );
            continue;
        }

        if let Some(stem) = filepath.as_str().strip_suffix("_ext.py") {
            let generated_path = Utf8PathBuf::try_from(format!("{stem}.py")).unwrap();
            assert!(
                generated_path.exists(),
                "Found orphaned {filepath} with no matching {generated_path}"
            );
            continue;
        }

        if let Some(stem) = filepath.as_str().strip_suffix("_ext.cpp") {
            let generated_hpp_path = Utf8PathBuf::try_from(format!("{stem}.hpp")).unwrap();
            assert!(
                generated_hpp_path.exists(),
                "Found orphaned {filepath} with no matching {generated_hpp_path}"
            );
            continue;
        }

        if !filepaths.contains(&filepath) {
            re_log::info!("Removing {filepath:?}");
            if let Err(err) = std::fs::remove_file(&filepath) {
                panic!("Failed to remove {filepath:?}: {err}");
            }
        }
    }
}

/// Write file if any changes were made and ensure folder hierarchy exists.
pub fn write_file(filepath: &Utf8PathBuf, source: &str) {
    if let Ok(existing) = std::fs::read_to_string(filepath) {
        if existing == source {
            // Don't touch the timestamp unnecessarily
            return;
        }
    }

    let parent_dir = filepath.parent().unwrap();
    std::fs::create_dir_all(parent_dir)
        .unwrap_or_else(|err| panic!("Failed to create dir {parent_dir:?}: {err}"));

    std::fs::write(filepath, source)
        .unwrap_or_else(|err| panic!("Failed to write file {filepath:?}: {err}"));
}
