use std::path::Path;

use super::{Context, DocumentData, DocumentKind};
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
        let url = format!("https://rerun.io/docs/{path}");
        let (frontmatter, body) = parse_docs_frontmatter(&entry)?;

        ctx.push(DocumentData {
            kind: DocumentKind::Docs,
            title: frontmatter.title,
            hidden_tags: vec![],
            tags: vec![],
            content: body,
            url,
        });
    }

    ctx.finish_progress_bar(progress);

    Ok(())
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
