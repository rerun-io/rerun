use std::path::Path;

use camino::Utf8PathBuf;

use super::{Context, DocumentData, DocumentKind, strip_html_tags};
use crate::build_search_index::util::ProgressBarExt as _;

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("docs");

    let snippets_root = ctx
        .workspace_root()
        .join("docs")
        .join("snippets")
        .join("all");
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
        let (frontmatter, raw_body) = parse_docs_frontmatter(&entry)?;
        // Inline `snippet:` build directives so sections whose only content is
        // a snippet carry the snippet's docstring and code, not an opaque
        // directive line.
        let body = resolve_snippets(&raw_body, &snippets_root);

        // Migration guides are special: their bodies mention every renamed API
        // and command ("log_image", "serve_grpc", "cargo install … protoc"),
        // which made them outrank the docs people actually want — a migration
        // note beat "Install Rerun" for "how to install". The deboost can't
        // stop it (weight is ranked after the matching rules). So index each
        // migration guide by TITLE ONLY: still findable by version or name
        // ("migrating from 0.25 to 0.26"), but it no longer competes on the
        // common terms in its body.
        let is_migration = path.starts_with("reference/migration");

        // Other pages are split into one document per `##` section and per
        // substantial `###` subsection (see `page_documents`), so a match deep
        // in a long page links to the right anchor and is ranked/excerpted on
        // that section's own text. All documents from a page share its `page`
        // value, so search shows at most one section per page.
        let parts = if is_migration {
            vec![DocPart {
                title: frontmatter.title.clone(),
                anchor: None,
                content: String::new(),
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

            // Ranking weight (used by the `weight:desc` rule): a page's own
            // intro represents the page (10) and should beat another page's
            // niche section (9) for a bare term, while sections still beat
            // examples (8) and API symbols (3). Migration titles sit below
            // examples (6).
            let weight = if is_migration {
                6
            } else if part.anchor.is_none() {
                10
            } else {
                9
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

/// Replace `snippet: <ref>` build directives with the referenced snippet's
/// source. Many archetype and how-to sections (e.g. "Update an image over
/// time") contain nothing but a heading and a snippet directive; the website
/// expands them, but the raw markdown leaves search with the opaque directive
/// line as its only content (and excerpt). Python is preferred because these
/// snippets lead with a descriptive docstring that makes an ideal excerpt.
fn resolve_snippets(body: &str, snippets_root: &camino::Utf8Path) -> String {
    let mut out = String::with_capacity(body.len());
    for line in body.lines() {
        if let Some(reference) = line.trim().strip_prefix("snippet:")
            && let Some(code) = load_snippet(snippets_root, reference.trim())
        {
            out.push_str(&code);
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Load a snippet by reference (`path/name` or `path/name[region]`), preferring
/// the Python source. Returns the snippet text with region markers removed,
/// or just the named region if one was requested.
fn load_snippet(root: &camino::Utf8Path, reference: &str) -> Option<String> {
    let (rel, region) = match reference.split_once('[') {
        Some((p, r)) => (p.trim(), Some(r.trim_end_matches(']').trim())),
        None => (reference, None),
    };
    for ext in ["py", "cpp", "rs"] {
        let path: Utf8PathBuf = root.join(format!("{rel}.{ext}"));
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Some(extract_region(&text, region));
        }
    }
    None
}

/// Strip `# region:` / `// region:` markers. If `region` is given, keep only
/// the lines inside the matching `region`/`endregion` block.
fn extract_region(text: &str, region: Option<&str>) -> String {
    let is_marker = |line: &str, kind: &str| {
        let t = line.trim();
        let t = t
            .strip_prefix("# ")
            .or_else(|| t.strip_prefix("// "))
            .unwrap_or(t);
        t.strip_prefix(kind)
            .map(|name| name.trim_start_matches(':').trim().to_owned())
    };

    let mut out = String::new();
    let mut in_target = region.is_none();
    for line in text.lines() {
        if let Some(name) = is_marker(line, "region") {
            if region == Some(name.as_str()) {
                in_target = true;
            }
            continue; // never keep the marker line itself
        }
        if let Some(name) = is_marker(line, "endregion") {
            if region == Some(name.as_str()) {
                in_target = false;
            }
            continue;
        }
        if in_target {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
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
    fn region_extraction_strips_markers_and_scopes() {
        use super::extract_region;
        let src = "\
header line
# region: setup
import rerun as rr
# endregion: setup
# region: ingest
rr.log(\"x\", data)
# endregion: ingest
";
        // No region requested: whole file minus marker lines.
        let all = extract_region(src, None);
        assert!(all.contains("header line"));
        assert!(all.contains("import rerun"));
        assert!(all.contains("rr.log"));
        assert!(!all.contains("region:"));
        // A named region: only its body.
        let ingest = extract_region(src, Some("ingest"));
        assert!(ingest.contains("rr.log"));
        assert!(!ingest.contains("import rerun"));
        assert!(!ingest.contains("header line"));
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
