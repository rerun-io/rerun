//! Helpers common to all codegen passes.

use std::collections::BTreeSet;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use itertools::Itertools as _;

use crate::{Docs, Reporter};

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

#[derive(Clone)]
pub struct ExampleInfo<'a> {
    /// The snake_case name of the example.
    ///
    /// Used with `code-example:`, `std::fs::read_to_string`, etc.
    pub name: &'a str,

    /// The human-readable name of the example.
    pub title: Option<&'a str>,

    /// A screenshot of the example.
    pub image: Option<ImageUrl<'a>>,

    /// If true, use this example only for the manual, not for documentation embedded in the emitted code.
    pub exclude_from_api_docs: bool,
}

impl<'a> ExampleInfo<'a> {
    /// Parses e.g.  `// \example example_name title="Example Title" image="https://www.example.com/img.png"`
    pub fn parse(tag_content: &'a impl AsRef<str>) -> Self {
        fn mono(tag_content: &str) -> ExampleInfo<'_> {
            fn find_keyed<'a>(tag: &str, args: &'a str) -> Option<&'a str> {
                let mut prev_end = 0;
                loop {
                    if prev_end + tag.len() + "=\"\"".len() >= args.len() {
                        return None;
                    }
                    let key_start = prev_end + args[prev_end..].find(tag)?;
                    let key_end = key_start + tag.len();
                    if !args[key_end..].starts_with("=\"") {
                        prev_end = key_end;
                        continue;
                    };
                    let value_start = key_end + "=\"".len();
                    let Some(mut value_end) = args[value_start..].find('"') else {
                        prev_end = value_start;
                        continue;
                    };
                    value_end += value_start;
                    return Some(&args[value_start..value_end]);
                }
            }

            let tag_content = tag_content.trim();
            let (name, args) = tag_content
                .split_once(' ')
                .map_or((tag_content, None), |(a, b)| (a, Some(b)));

            let (mut title, mut image, mut exclude_from_api_docs) = (None, None, false);

            if let Some(args) = args {
                let args = args.trim();

                exclude_from_api_docs = args.contains("!api");
                let args = if let Some(args_without_api_prefix) = args.strip_prefix("!api") {
                    args_without_api_prefix.trim()
                } else {
                    args
                };

                if args.starts_with('"') {
                    // \example example_name "Example Title"
                    title = args.strip_prefix('"').and_then(|v| v.strip_suffix('"'));
                } else {
                    // \example example_name title="Example Title" image="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1200w.png"
                    title = find_keyed("title", args);
                    image = find_keyed("image", args).map(ImageUrl::parse);
                }
            }

            ExampleInfo {
                name,
                title,
                image,
                exclude_from_api_docs,
            }
        }

        mono(tag_content.as_ref())
    }
}

#[derive(Clone, Copy)]
pub enum ImageUrl<'a> {
    /// A URL with our specific format:
    ///
    /// ```text,ignore
    /// https://static.rerun.io/{name}/{image_hash}/{size}.{ext}
    /// ```
    ///
    /// The `https://static.rerun.io/` base is optional.
    Rerun(RerunImageUrl<'a>),

    /// Any other URL.
    Other(&'a str),
}

impl ImageUrl<'_> {
    pub fn parse(s: &str) -> ImageUrl<'_> {
        RerunImageUrl::parse(s).map_or(ImageUrl::Other(s), ImageUrl::Rerun)
    }

    pub fn image_stack(&self) -> Vec<String> {
        match self {
            ImageUrl::Rerun(rerun) => rerun.image_stack(),
            ImageUrl::Other(url) => {
                vec![format!(r#"<img src="{url}">"#)]
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct RerunImageUrl<'a> {
    pub name: &'a str,
    pub hash: &'a str,
    pub max_width: Option<u16>,
    pub extension: &'a str,
}

impl RerunImageUrl<'_> {
    /// Parses e.g. `https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1200w.png`
    pub fn parse(s: &str) -> Option<RerunImageUrl<'_>> {
        let path = s.strip_prefix("https://static.rerun.io/")?;
        // We're on a `static.rerun.io` URL, so we can make assumptions about the format:

        let (rest, extension) = path.rsplit_once('.')?;
        let mut parts = rest.split('/');
        let name = parts.next()?;
        let hash = parts.next()?;
        // Note: failure to parse here means we fall back to showing only the `full` size.
        let max_width = parts.next()?;
        let max_width = max_width.strip_suffix('w').and_then(|v| v.parse().ok());
        if parts.next().is_some() {
            return None;
        }

        Some(RerunImageUrl {
            name,
            hash,
            max_width,
            extension,
        })
    }

    pub fn image_stack(&self) -> Vec<String> {
        const WIDTHS: [u16; 4] = [480, 768, 1024, 1200];

        // Don't let the images take up too much space on the page.
        let desired_with = Some(640);

        let RerunImageUrl {
            name,
            hash,
            max_width,
            extension,
        } = *self;

        let mut stack = vec!["<center>".into(), "<picture>".into()];
        if let Some(max_width) = max_width {
            for width in WIDTHS {
                if width > max_width {
                    break;
                }
                stack.push(format!(
                    r#"  <source media="(max-width: {width}px)" srcset="https://static.rerun.io/{name}/{hash}/{width}w.{extension}">"#
                ));
            }
        }

        if let Some(desired_with) = desired_with {
            stack.push(format!(
                r#"  <img src="https://static.rerun.io/{name}/{hash}/full.{extension}" width="{desired_with}">"#
            ));
        } else {
            stack.push(format!(
                r#"  <img src="https://static.rerun.io/{name}/{hash}/full.{extension}">"#
            ));
        }
        stack.push("</picture>".into());
        stack.push("</center>".into());

        stack
    }
}

pub struct Example<'a> {
    pub base: ExampleInfo<'a>,
    pub lines: Vec<String>,
}

pub fn collect_examples_for_api_docs<'a>(
    docs: &'a Docs,
    extension: &str,
    required: bool,
) -> anyhow::Result<Vec<Example<'a>>> {
    let mut out = Vec::new();

    if let Some(examples) = docs.tagged_docs.get("example") {
        let base_path = crate::rerun_workspace_path().join("docs/code-examples");

        for base @ ExampleInfo {
            name,
            exclude_from_api_docs,
            ..
        } in examples.iter().map(ExampleInfo::parse)
        {
            if exclude_from_api_docs {
                continue;
            }

            let path = base_path.join(format!("{name}.{extension}"));
            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) if !required => continue,
                Err(err) => {
                    return Err(err).with_context(|| format!("couldn't open code example {path:?}"))
                }
            };
            let mut content = content.split('\n').map(String::from).collect_vec();
            // trim trailing blank lines
            while content.last().is_some_and(is_blank) {
                content.pop();
            }

            out.push(Example {
                base,
                lines: content,
            });
        }
    }

    Ok(out)
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
pub fn remove_old_files_from_folder(
    reporter: &Reporter,
    folder_path: Utf8PathBuf,
    filepaths: &BTreeSet<Utf8PathBuf>,
) {
    re_tracing::profile_function!();
    re_log::debug!("Checking for old files in {folder_path}");

    let iter = std::fs::read_dir(folder_path).ok();
    if iter.is_none() {
        return;
    }

    for entry in iter.unwrap().flatten() {
        if entry.file_type().unwrap().is_dir() {
            continue;
        }
        let filepath = Utf8PathBuf::try_from(entry.path()).unwrap();

        if let Some(stem) = filepath.as_str().strip_suffix("_ext.rs") {
            let generated_path = Utf8PathBuf::try_from(format!("{stem}.rs")).unwrap();
            if !generated_path.exists() {
                reporter.error(
                    filepath.as_str(),
                    "",
                    format!("Found orphaned {filepath} with no matching {generated_path}"),
                );
            }
            continue;
        }

        if let Some(stem) = filepath.as_str().strip_suffix("_ext.py") {
            let generated_path = Utf8PathBuf::try_from(format!("{stem}.py")).unwrap();
            if !generated_path.exists() {
                reporter.error(
                    filepath.as_str(),
                    "",
                    format!("Found orphaned {filepath} with no matching {generated_path}"),
                );
            }
            continue;
        }

        if let Some(stem) = filepath.as_str().strip_suffix("_ext.cpp") {
            let generated_hpp_path = Utf8PathBuf::try_from(format!("{stem}.hpp")).unwrap();
            if !generated_hpp_path.exists() {
                reporter.error(
                    filepath.as_str(),
                    "",
                    format!("Found orphaned {filepath} with no matching {generated_hpp_path}"),
                );
            }
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
