use re_build_examples::example::ExamplesManifest;
use re_build_examples::example::Language;
use re_build_examples::Example;

use crate::ingest::DocumentData;
use crate::ingest::DocumentKind;

use super::Context;

const LANGUAGES: &[Language] = &[Language::Python, Language::Rust, Language::Cpp];

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("examples");

    let manifest = ExamplesManifest::load(ctx.workspace_root())?;

    for (category_name, category) in &manifest.categories {
        ctx.push(DocumentData {
            kind: DocumentKind::Examples,
            title: category.title.clone(),
            hidden_tags: vec![],
            tags: vec![],
            content: category.prelude.clone(),
            url: format!("https://rerun.io/examples/{category_name}"),
        });

        for example_name in &category.examples {
            for language in LANGUAGES.iter().copied() {
                progress.set_message(format!(
                    "{category_name}/{example_name}.{}",
                    language.extension()
                ));

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
                });

                break;
            }
        }
    }

    ctx.finish_progress_bar(progress);

    Ok(())
}
