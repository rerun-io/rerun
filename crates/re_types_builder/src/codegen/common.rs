//! Helpers common to all codegen passes.

use std::collections::BTreeSet;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use itertools::Itertools as _;

use crate::{Docs, GeneratedFiles, Reporter};

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
    /// Used with `snippet:`, `std::fs::read_to_string`, etc.
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

impl<'a> ImageUrl<'a> {
    pub fn parse(s: &str) -> ImageUrl<'_> {
        RerunImageUrl::parse(s).map_or(ImageUrl::Other(s), ImageUrl::Rerun)
    }

    /// Try to generate a `<picture>` stack, falling back to a single `<img>` element.
    pub fn image_stack(&self) -> ImageStack<'_> {
        ImageStack {
            url: self,
            width: None,
            snippet_id: None,
            center: false,
        }
    }
}

pub struct ImageStack<'a> {
    url: &'a ImageUrl<'a>,
    width: Option<u16>,
    snippet_id: Option<SnippetId<'a>>,
    center: bool,
}

impl<'a> ImageStack<'a> {
    /// Set the `width` attribute of the image.
    #[inline]
    pub fn width(mut self, v: u16) -> Self {
        self.width = Some(v);
        self
    }

    /// Whether or not the image should be wrapped in `<center>`.
    #[inline]
    pub fn center(mut self) -> Self {
        self.center = true;
        self
    }

    /// Set the snippet ID.
    ///
    /// If set, the resulting `<picture>` element will have the `data-inline-viewr`
    /// attribute set with the value of this ID.
    /// `data-inline-viewer` is not set for `<img>` elements.
    #[inline]
    pub fn snippet_id(mut self, id: &'a str) -> Self {
        self.snippet_id = Some(SnippetId(id));
        self
    }

    pub fn finish(self) -> Vec<String> {
        match self.url {
            ImageUrl::Rerun(rerun) => rerun.image_stack(self.snippet_id, self.width, self.center),
            ImageUrl::Other(url) => {
                vec![format!(r#"<img src="{url}">"#)]
            }
        }
    }
}

pub struct SnippetId<'a>(pub &'a str);

impl<'a> std::fmt::Display for SnippetId<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "snippets/{}", self.0)
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

    pub fn image_stack(
        &self,
        snippet_id: Option<SnippetId<'_>>,
        desired_width: Option<u16>,
        center: bool,
    ) -> Vec<String> {
        const WIDTHS: [u16; 4] = [480, 768, 1024, 1200];

        let RerunImageUrl {
            name,
            hash,
            max_width,
            extension,
        } = *self;

        let mut stack = vec![];

        if center {
            stack.push("<center>".into());
        }

        match snippet_id {
            Some(id) => stack.push(format!(r#"<picture data-inline-viewer="{id}">"#)),
            None => stack.push("<picture>".into()),
        }

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

        if let Some(desired_width) = desired_width {
            stack.push(format!(
                r#"  <img src="https://static.rerun.io/{name}/{hash}/full.{extension}" width="{desired_width}">"#
            ));
        } else {
            stack.push(format!(
                r#"  <img src="https://static.rerun.io/{name}/{hash}/full.{extension}">"#
            ));
        }
        stack.push("</picture>".into());

        if center {
            stack.push("</center>".into());
        }

        stack
    }

    pub fn markdown_tag(&self) -> String {
        let RerunImageUrl {
            name,
            hash,
            max_width: _,
            extension,
        } = *self;
        format!("![image](https://static.rerun.io/{name}/{hash}/full.{extension})")
    }
}

pub struct Example<'a> {
    pub base: ExampleInfo<'a>,
    pub lines: Vec<String>,
}

pub fn collect_snippets_for_api_docs<'a>(
    docs: &'a Docs,
    extension: &str,
    required: bool,
) -> anyhow::Result<Vec<Example<'a>>> {
    let mut out = Vec::new();

    if let Some(examples) = docs.tagged_docs.get("example") {
        let base_path = crate::rerun_workspace_path().join("docs/snippets/all");

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
                    return Err(err).with_context(|| format!("couldn't open snippet {path:?}"))
                }
            };
            let mut content = content
                .split('\n')
                .map(String::from)
                .skip_while(|line| line.starts_with("//") || line.starts_with(r#"""""#)) // Skip leading comments.
                .skip_while(|line| line.trim().is_empty()) // Strip leading empty lines.
                .collect_vec();

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
    fn push_indented(
        &mut self,
        indent_level: usize,
        text: impl AsRef<str>,
        linefeeds: usize,
    ) -> &mut Self;

    fn push_unindented(&mut self, text: impl AsRef<str>, linefeeds: usize) -> &mut Self;
}

impl StringExt for String {
    fn push_indented(
        &mut self,
        indent_level: usize,
        text: impl AsRef<str>,
        linefeeds: usize,
    ) -> &mut Self {
        self.push_str(&indent::indent_all_by(indent_level * 4, text.as_ref()));
        self.push_str(&vec!["\n"; linefeeds].join(""));
        self
    }

    fn push_unindented(&mut self, text: impl AsRef<str>, linefeeds: usize) -> &mut Self {
        self.push_str(&unindent::unindent(text.as_ref()));
        self.push_str(&vec!["\n"; linefeeds].join(""));
        self
    }
}

/// Remove orphaned files in all directories present in `files`.
pub fn remove_orphaned_files(reporter: &Reporter, files: &GeneratedFiles) {
    re_tracing::profile_function!();

    let folder_paths: BTreeSet<_> = files
        .keys()
        .filter_map(|filepath| filepath.parent())
        .collect();

    for folder_path in folder_paths {
        re_log::trace!("Checking for orphaned files in {folder_path}");

        let iter = std::fs::read_dir(folder_path).ok();
        if iter.is_none() {
            re_log::debug!("Skipping orphan check in {folder_path}: not a folder (?)");
            continue;
        }

        for entry in iter.unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                continue;
            }
            let filepath = Utf8PathBuf::try_from(entry.path()).unwrap();

            if let Some(stem) = filepath.as_str().strip_suffix("_ext.rs") {
                let generated_path = Utf8PathBuf::from(format!("{stem}.rs"));
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
                let generated_path = Utf8PathBuf::from(format!("{stem}.py"));
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
                let generated_hpp_path = Utf8PathBuf::from(format!("{stem}.hpp"));
                if !generated_hpp_path.exists() {
                    reporter.error(
                        filepath.as_str(),
                        "",
                        format!("Found orphaned {filepath} with no matching {generated_hpp_path}"),
                    );
                }
                continue;
            }

            if !files.contains_key(&filepath) {
                re_log::info!("Removing {filepath:?}");
                if let Err(err) = std::fs::remove_file(&filepath) {
                    panic!("Failed to remove {filepath:?}: {err}");
                }
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
