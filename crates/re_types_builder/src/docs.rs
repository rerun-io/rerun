use std::collections::{BTreeMap, HashSet};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools as _;

/// A high-level representation of a flatbuffers object's documentation.
#[derive(Debug, Clone)]
pub struct Docs {
    /// General documentation for the object.
    ///
    /// Each entry in the vector is a line of comment,
    /// excluding the leading space end trailing newline,
    /// i.e. the `COMMENT` from `/// COMMENT\n`
    ///
    /// See also [`Docs::tagged_docs`].
    doc: Vec<String>,

    /// Tagged documentation for the object.
    ///
    /// Each entry in the vector is a line of comment,
    /// excluding the leading space end trailing newline,
    /// i.e. the `COMMENT` from `/// \py COMMENT\n`
    ///
    /// E.g. the following will be associated with the `py` tag:
    /// ```flatbuffers
    /// /// \py Something something about how this fields behave in python.
    /// my_field: uint32,
    /// ```
    ///
    /// See also [`Docs::doc`].
    tagged_docs: BTreeMap<String, Vec<String>>,

    /// Contents of all the files included using `\include:<path>`.
    included_files: BTreeMap<Utf8PathBuf, String>,
}

impl Docs {
    pub fn from_raw_docs(
        filepath: &Utf8Path,
        docs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&'_ str>>>,
    ) -> Self {
        let mut included_files = BTreeMap::default();

        let include_file = |included_files: &mut BTreeMap<_, _>, raw_path: &str| {
            let path: Utf8PathBuf = raw_path
                .parse()
                .with_context(|| format!("couldn't parse included path: {raw_path:?}"))
                .unwrap();

            let path = filepath.parent().unwrap().join(path);

            included_files
                .entry(path.clone())
                .or_insert_with(|| {
                    std::fs::read_to_string(&path)
                        .with_context(|| {
                            format!("couldn't parse read file at included path: {path:?}")
                        })
                        .unwrap()
                })
                .clone()
        };

        // language-agnostic docs
        let doc = docs
            .into_iter()
            .flat_map(|doc| doc.into_iter())
            // NOTE: discard tagged lines!
            .filter(|line| !line.trim().starts_with('\\'))
            .flat_map(|line| {
                assert!(!line.ends_with('\n'));
                assert!(!line.ends_with('\r'));

                if let Some((_, path)) = line.split_once("\\include:") {
                    include_file(&mut included_files, path)
                        .lines()
                        .map(|line| line.to_owned())
                        .collect_vec()
                } else if let Some(line) = line.strip_prefix(' ') {
                    // Removed space between `///` and comment.
                    vec![line.to_owned()]
                } else {
                    assert!(
                        line.is_empty(),
                        "{filepath}: Comments should start with a single space; found {line:?}"
                    );
                    vec![line.to_owned()]
                }
            })
            .collect::<Vec<_>>();

        // tagged docs, e.g. `\py this only applies to python!`
        let tagged_docs = {
            let tagged_lines = docs
                .into_iter()
                .flat_map(|doc| doc.into_iter())
                // NOTE: discard _un_tagged lines!
                .filter_map(|line| {
                    let trimmed = line.trim();
                    trimmed.starts_with('\\').then(|| {
                        let tag = trimmed.split_whitespace().next().unwrap();
                        let line = &trimmed[tag.len()..];
                        let tag = tag[1..].to_owned();
                        if let Some(line) = line.strip_prefix(' ') {
                            // Removed space between tag and comment.
                            (tag, line.to_owned())
                        } else {
                            assert!(line.is_empty());
                            (tag, String::default())
                        }
                    })
                })
                .flat_map(|(tag, line)| {
                    if let Some((_, path)) = line.split_once("\\include:") {
                        include_file(&mut included_files, path)
                            .lines()
                            .map(|line| (tag.clone(), line.to_owned()))
                            .collect_vec()
                    } else {
                        vec![(tag, line)]
                    }
                })
                .collect::<Vec<_>>();

            let all_tags: HashSet<_> = tagged_lines.iter().map(|(tag, _)| tag).collect();
            let mut tagged_docs = BTreeMap::new();

            for cur_tag in all_tags {
                tagged_docs.insert(
                    cur_tag.clone(),
                    tagged_lines
                        .iter()
                        .filter(|(tag, _)| cur_tag == tag)
                        .map(|(_, line)| line.clone())
                        .collect(),
                );
            }

            tagged_docs
        };

        Self {
            doc,
            tagged_docs,
            included_files,
        }
    }

    /// Get all doc lines that start with the given tag.
    ///
    /// For instance, pass `"example"` to get all lines that start with `"\example"`.
    pub fn doc_lines_tagged(&self, tag: &str) -> Vec<&str> {
        if let Some(lines) = self.tagged_docs.get(tag) {
            lines.iter().map(|s| s.as_str()).collect()
        } else {
            Vec::new()
        }
    }

    /// Get all doc lines that are untagged, or match any of the given tags.
    ///
    /// For instance, pass `["py"]` to get all lines that are untagged or starta with `"\python"`.
    pub fn doc_lines_for_untagged_and(&self, tags: &[&str]) -> Vec<String> {
        let mut lines = self.doc.clone();

        for tag in tags {
            lines.extend(
                self.tagged_docs
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
}
