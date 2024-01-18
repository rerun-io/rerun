use std::path::Path;

use super::{Context, DocumentData, DocumentKind};

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
        progress.set_message(path.clone());
        let url = format!("https://rerun.io/docs/{path}");
        let (frontmatter, body) = parse_docs_frontmatter(&entry)?;

        ctx.push(DocumentData {
            kind: DocumentKind::Docs,
            title: frontmatter.title,
            tags: vec![],
            content: body,
            url,
        });
    }

    ctx.finish_progress_bar(progress);

    Ok(())
}

#[derive(serde::Deserialize)]
struct DocsFrontmatter {
    title: String,
}

fn parse_docs_frontmatter<P: AsRef<Path>>(path: P) -> anyhow::Result<(DocsFrontmatter, String)> {
    const START: &str = "---";
    const END: &str = "---";

    let path = path.as_ref();
    let content = std::fs::read_to_string(path)?;

    let Some(start) = content.find(START) else {
        anyhow::bail!("\"{}\" is missing frontmatter", path.display())
    };
    let start = start + START.len();

    let Some(end) = content[start..].find(END) else {
        anyhow::bail!(
            "\"{}\" has invalid frontmatter: missing {END:?} terminator",
            path.display()
        );
    };
    let end = start + end;

    let frontmatter: DocsFrontmatter =
        serde_yaml::from_str(content[start..end].trim()).map_err(|err| {
            anyhow::anyhow!(
                "Failed to parse YAML metadata of {:?}: {err}",
                path.parent().unwrap().file_name().unwrap()
            )
        })?;

    Ok((frontmatter, content[end + END.len()..].trim().to_owned()))
}
