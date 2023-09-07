//! Helpers common to all codegen passes.

use std::collections::BTreeSet;

use camino::Utf8PathBuf;

use crate::Docs;

/// Retrieves the global and tagged documentation from a [`Docs`] object.
pub fn get_documentation(docs: &Docs, tags: &[&str]) -> Vec<String> {
    fn trim_mono_start_whitespace_if_needed(line: &str) -> &str {
        if line.chars().next().map_or(false, |c| c.is_whitespace()) {
            // NOTE: don't trim! only that very specific space should go away
            &line[1..]
        } else {
            line
        }
    }

    let mut lines = Vec::new();

    for line in &docs.doc {
        lines.push(trim_mono_start_whitespace_if_needed(line).to_owned());
    }

    let empty = Vec::new();
    for tag in tags {
        for line in docs.tagged_docs.get(*tag).unwrap_or(&empty) {
            lines.push(trim_mono_start_whitespace_if_needed(line).to_owned());
        }
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
    for entry in std::fs::read_dir(folder_path).unwrap().flatten() {
        let filepath = Utf8PathBuf::try_from(entry.path()).unwrap();

        if let Some(stem) = filepath.as_str().strip_suffix("_ext.rs") {
            let generated_path = Utf8PathBuf::try_from(format!("{stem}.rs")).unwrap();
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
            std::fs::remove_file(filepath).ok();
        }
    }
}

/// Write file if any changes were made and ensure folder hierarchy exists.
pub fn write_file(filepath: &Utf8PathBuf, source: String) {
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
