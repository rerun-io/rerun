use re_build_examples::example::ExamplesManifest;
use re_build_examples::example::Language;
use re_build_examples::Example;

use crate::ingest::DocumentData;
use crate::ingest::DocumentKind;

use super::Context;

const LANGUAGES: &[Language] = &[Language::Python, Language::Rust, Language::Cpp];

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let manifest = ExamplesManifest::load(ctx.workspace_root())?;

    // TODO: also generate a document for each category
    for (category_name, category) in &manifest.categories {
        ctx.push(DocumentData {
            kind: DocumentKind::Examples,
            title: category.title.clone(),
            tags: vec![],
            content: category.prelude.clone(),
            url: format!("https://rerun.io/examples/{category_name}"),
        });

        for example_name in &category.examples {
            for language in LANGUAGES.iter().copied() {
                let Some(example) = Example::load(ctx.workspace_root(), example_name, language)?
                else {
                    continue;
                };

                ctx.push(DocumentData {
                    kind: DocumentKind::Examples,
                    title: example.title,
                    tags: example.tags,
                    content: example.readme_body,
                    url: format!("https://rerun.io/examples/{category_name}/{example_name}"),
                });

                break;
            }
        }
    }

    Ok(())
}
