use std::path::Path;

use super::{Context, DocumentData, DocumentKind, strip_html_tags};
use crate::build_search_index::util::ProgressBarExt as _;

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("docs");

    let dir = ctx.workspace_root().join("docs").join("content");
    for entry in glob::glob(&format!("{dir}/**/*.md"))? {
        let entry = entry?;
        let path = entry
            .strip_prefix(&dir)?
            .with_extension("")
            .display()
            .to_string();
        progress.set(path.clone(), ctx.is_tty());
        let page_url = format!("https://rerun.io/docs/{path}");
        let (frontmatter, body) = parse_docs_frontmatter(&entry)?;

        // Migration guides mention every renamed API and command, which lets
        // them win ranking ties against the docs people actually search for
        // ("how to install" ranked a migration note above "Install Rerun").
        // Deboost them below examples; they remain findable for migration-
        // and version-shaped queries.
        let weight = if path.starts_with("reference/migration") {
            6
        } else {
            10
        };

        // One document per section (split on `##` headings), so a match deep
        // in a long page links to the right anchor and is ranked/excerpted on
        // the section's own text. All sections share the page's `page` value,
        // so search results show at most one section per page.
        for section in split_sections(&body) {
            let (title, url, page_title) = match &section.heading {
                None => (frontmatter.title.clone(), page_url.clone(), None),
                Some(heading) => (
                    display_title(heading),
                    format!("{page_url}#{}", heading_id(heading)),
                    Some(frontmatter.title.clone()),
                ),
            };

            ctx.push_weighted(
                DocumentData {
                    kind: DocumentKind::Docs,
                    title,
                    hidden_tags: vec![],
                    tags: vec![],
                    content: strip_html_tags(&section.content),
                    url,
                    page: Some(page_url.clone()),
                    page_title,
                    image: None,
                },
                weight,
            );
        }
    }

    ctx.finish_progress_bar(progress);

    Ok(())
}

struct Section {
    /// Raw heading text (without the leading `## `); `None` for the content
    /// before the first heading, which carries the page title.
    heading: Option<String>,
    content: String,
}

/// Split a markdown body into its intro and one section per `##` heading.
///
/// Fence-aware: `#` lines inside code blocks (e.g. Python comments) are
/// content, not headings. Deeper headings (`###`…) stay inside their parent
/// section — per-`##` granularity is what maps to one topic per result.
fn split_sections(body: &str) -> Vec<Section> {
    let mut sections = vec![Section {
        heading: None,
        content: String::new(),
    }];

    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }

        let heading = if in_fence {
            None
        } else {
            line.strip_prefix("## ")
        };
        match heading {
            Some(heading) if !heading.trim().is_empty() => {
                sections.push(Section {
                    heading: Some(heading.trim().to_owned()),
                    content: String::new(),
                });
            }
            _ => {
                let section = sections.last_mut().expect("never empty");
                section.content.push_str(line);
                section.content.push('\n');
            }
        }
    }

    sections.retain(|s| s.heading.is_some() || !s.content.trim().is_empty());
    sections
}

/// Anchor id for a heading. Must match `getHeadingId` in the website
/// (rerun-io/landing `src/lib/client/docs.ts`), which computes ids from the
/// raw heading text: lowercase, drop everything but `[a-z0-9 ]`, spaces
/// become dashes (consecutive spaces are NOT collapsed).
fn heading_id(raw_heading: &str) -> String {
    raw_heading
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Heading text as shown in search results: inline-code backticks dropped.
fn display_title(raw_heading: &str) -> String {
    raw_heading.replace('`', "")
}

struct DocsFrontmatter {
    title: String,
}

fn find_frontmatter_and_body(path: &Path) -> anyhow::Result<(String, String)> {
    let content = std::fs::read_to_string(path)?;

    const START: &str = "---";
    const END: &str = "---";

    let Some(start) = content.find(START) else {
        anyhow::bail!("{:?} is missing frontmatter", path.display())
    };
    let start = start + START.len();

    let Some(end) = content[start..].find(END) else {
        anyhow::bail!(
            "{:?} has invalid frontmatter: missing {END:?} terminator",
            path.display()
        );
    };
    let end = start + end;

    let frontmatter = content[start..end].trim().to_owned();
    let body = content[end + END.len()..].trim().to_owned();

    Ok((frontmatter, body))
}

fn parse_docs_frontmatter(path: &Path) -> anyhow::Result<(DocsFrontmatter, String)> {
    const TITLE_FIELD: &str = "title:";

    let (frontmatter, body) = find_frontmatter_and_body(path)?;

    // Parse `title: Some Title` and `title: "Some Title"` manually, to avoid depending on yaml.
    // If we want to add support for more fields, just switch our frontmatter to be toml, or json, or anything but yaml.

    let Some(title_start) = frontmatter.find(TITLE_FIELD) else {
        anyhow::bail!("{:?} is missing title field in frontmatter", path.display());
    };
    let title_start = title_start + TITLE_FIELD.len();
    let title_end = frontmatter[title_start..]
        .find('\n')
        .map_or(frontmatter.len(), |idx| title_start + idx);

    let title = frontmatter[title_start..title_end]
        .trim()
        .trim_matches('"')
        .to_owned();

    Ok((DocsFrontmatter { title }, body))
}

#[cfg(test)]
mod tests {
    use super::{heading_id, split_sections};

    #[test]
    fn heading_ids_match_the_website() {
        // See `getHeadingId` in rerun-io/landing src/lib/client/docs.ts.
        assert_eq!(heading_id("Installing the SDK"), "installing-the-sdk");
        assert_eq!(heading_id("Logging `Image` data"), "logging-image-data");
        assert_eq!(heading_id("C++ & Rust"), "c--rust");
        assert_eq!(heading_id("What's new?"), "whats-new");
    }

    #[test]
    fn fences_do_not_split_sections() {
        let body = "intro\n\n## First\n```py\n## not a heading\n```\nmore\n\n## Second\ntail\n";
        let sections = split_sections(body);
        let headings: Vec<_> = sections.iter().map(|s| s.heading.as_deref()).collect();
        assert_eq!(headings, vec![None, Some("First"), Some("Second")]);
        assert!(sections[1].content.contains("## not a heading"));
    }
}
