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

        // Migration guides are special: their headings are bare renamed-API
        // names ("log_image", "serve_grpc") that collide with real queries,
        // and the deboost below can't stop a tight title match (weight is
        // ranked after the matching rules). So index each migration guide as a
        // single page-level document — findable by version or name, but not
        // competing section-by-section — and deboost it below examples.
        let is_migration = path.starts_with("reference/migration");
        let weight = if is_migration { 6 } else { 10 };

        // Other pages are split into one document per `##` section and per
        // substantial `###` subsection (see `page_documents`), so a match deep
        // in a long page links to the right anchor and is ranked/excerpted on
        // that section's own text. All documents from a page share its `page`
        // value, so search shows at most one section per page.
        let parts = if is_migration {
            vec![DocPart {
                title: frontmatter.title.clone(),
                anchor: None,
                content: body.clone(),
            }]
        } else {
            page_documents(&frontmatter.title, &body)
        };

        for part in parts {
            let (url, page_title) = match &part.anchor {
                None => (page_url.clone(), None),
                Some(anchor) => (
                    format!("{page_url}#{anchor}"),
                    Some(frontmatter.title.clone()),
                ),
            };

            ctx.push_weighted(
                DocumentData {
                    kind: DocumentKind::Docs,
                    title: part.title,
                    hidden_tags: vec![],
                    tags: vec![],
                    content: strip_html_tags(&part.content),
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

/// A subsection with fewer than this many words is folded back into its parent
/// `##` section rather than indexed on its own — otherwise a one-line `###`
/// could win a page's single result slot (`distinctAttribute: page`) over a
/// richer section via an exact title match. ~22% of `###` headings in the docs
/// are this thin.
const MIN_SUBSECTION_WORDS: usize = 15;

/// One emitted search document for a docs page.
struct DocPart {
    /// Display title: the page title for the intro, else the heading text.
    title: String,
    /// Heading anchor (`#some-heading`); `None` for the page intro.
    anchor: Option<String>,
    content: String,
}

/// Split a docs page into search documents, one per `##` section and per
/// substantial `###` subsection, plus the page intro.
///
/// Pages like "Send partial updates" put many sibling topics ("Updating a
/// point cloud over time", …) under a single `##`, so `##`-only splitting
/// would link all of them to the section top with a generic title. We recurse
/// one level: each substantial `###` becomes its own document (with its own
/// anchor and title), while thin subsections and the section's own intro stay
/// with the parent `##` document. We stop at `###` — deeper headings are
/// almost always too thin to index standalone.
fn page_documents(page_title: &str, body: &str) -> Vec<DocPart> {
    let mut parts = Vec::new();

    for section in split_at_heading(body, 2) {
        match section.heading {
            // Page intro (before the first `##`): represents the page itself.
            None => {
                if !section.content.trim().is_empty() {
                    parts.push(DocPart {
                        title: page_title.to_owned(),
                        anchor: None,
                        content: section.content,
                    });
                }
            }
            Some(h2) => {
                // The `##` document accumulates its own intro plus any thin
                // subsections folded back in.
                let mut h2_content = String::new();
                let mut children = Vec::new();

                for sub in split_at_heading(&section.content, 3) {
                    match sub.heading {
                        None => h2_content.push_str(&sub.content),
                        Some(h3) => {
                            if word_count(&sub.content) >= MIN_SUBSECTION_WORDS {
                                children.push(DocPart {
                                    title: display_title(&h3),
                                    anchor: Some(heading_id(&h3)),
                                    content: sub.content,
                                });
                            } else {
                                // Keep the thin subsection searchable under its
                                // parent, including its heading text.
                                h2_content.push_str(&h3);
                                h2_content.push('\n');
                                h2_content.push_str(&sub.content);
                            }
                        }
                    }
                }

                // Emit the `##` document unless it is a pure container whose
                // children already represent it (e.g. "## Examples" with only
                // substantial subsections and no prose of its own).
                if !h2_content.trim().is_empty() || children.is_empty() {
                    parts.push(DocPart {
                        title: display_title(&h2),
                        anchor: Some(heading_id(&h2)),
                        content: h2_content,
                    });
                }
                parts.extend(children);
            }
        }
    }

    parts
}

struct Section {
    /// Raw heading text (without the leading marker); `None` for the content
    /// before the first heading at this level.
    heading: Option<String>,
    content: String,
}

/// Split a markdown body at headings of exactly `level` (`## ` or `### `).
///
/// Fence-aware: heading-like lines inside code blocks (e.g. Python `#`
/// comments) are content, not headings.
fn split_at_heading(body: &str, level: usize) -> Vec<Section> {
    let marker = format!("{} ", "#".repeat(level));
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
            line.strip_prefix(marker.as_str())
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

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
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
    use super::{heading_id, page_documents, split_at_heading};

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
        let sections = split_at_heading(body, 2);
        let headings: Vec<_> = sections.iter().map(|s| s.heading.as_deref()).collect();
        assert_eq!(headings, vec![None, Some("First"), Some("Second")]);
        assert!(sections[1].content.contains("## not a heading"));
    }

    #[test]
    fn substantial_subsections_become_their_own_documents() {
        // A "## Examples" container whose `###` topics each carry real prose.
        let body = "\
## Examples

### Updating a point cloud over time
Log every frame into the same entity so the viewer can scrub across time, \
using a single column-oriented send for the whole sequence of positions.

### Updating an image over time
Stream frames into one image entity and the viewer plays them back as video, \
again with a single send covering the entire timeline of frames.
";
        let parts = page_documents("Send partial updates", body);
        let titles: Vec<_> = parts.iter().map(|p| p.title.as_str()).collect();
        // Pure container `## Examples` is dropped; each `###` stands alone.
        assert_eq!(
            titles,
            vec![
                "Updating a point cloud over time",
                "Updating an image over time",
            ]
        );
        assert_eq!(
            parts[0].anchor.as_deref(),
            Some("updating-a-point-cloud-over-time")
        );
    }

    #[test]
    fn thin_subsections_fold_into_their_parent() {
        let body = "\
## Reference

Some real introductory prose that comfortably clears the word threshold so the
section is indexed as its own document with its own anchor.

### Tiny
See above.
";
        let parts = page_documents("API", body);
        assert_eq!(parts.len(), 1, "thin `### Tiny` must not be its own doc");
        assert_eq!(parts[0].title, "Reference");
        // The thin subsection's heading and body stay searchable under the parent.
        assert!(parts[0].content.contains("Tiny"));
    }
}
