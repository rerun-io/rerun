use super::{Context, DocumentData, DocumentKind};
use crate::build_examples::{Example, ExamplesManifest, Language};
use crate::build_search_index::util::ProgressBarExt as _;

const LANGUAGES: &[Language] = &[Language::Python, Language::Rust, Language::Cpp];

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("examples");

    let manifest = ExamplesManifest::load(ctx.workspace_root())?;

    // Examples may be listed under multiple categories; only index the first
    // occurrence so a single example doesn't show up twice in search results.
    let mut seen = std::collections::HashSet::new();

    for (category_name, category) in &manifest.categories {
        ctx.push(DocumentData {
            kind: DocumentKind::Examples,
            title: category.title.clone(),
            hidden_tags: vec![],
            tags: vec![],
            content: category.prelude.clone(),
            url: format!("https://rerun.io/examples/{category_name}"),
            image: None,
        });

        for example_name in &category.examples {
            if !seen.insert(example_name) {
                continue;
            }
            for language in LANGUAGES.iter().copied() {
                progress.set(
                    format!("{category_name}/{example_name}.{}", language.extension()),
                    ctx.is_tty(),
                );

                let Some(example) = Example::load(ctx.workspace_root(), example_name, language)?
                else {
                    continue;
                };

                ctx.push(DocumentData {
                    kind: DocumentKind::Examples,
                    title: example.title,
                    hidden_tags: vec![],
                    tags: example.tags,
                    content: example.readme_body,
                    url: format!("https://rerun.io/examples/{category_name}/{example_name}"),
                    image: (!example.thumbnail_url.is_empty()).then_some(example.thumbnail_url),
                });

                break;
            }
        }
    }

    ctx.finish_progress_bar(progress);

    Ok(())
}
