use crate::codegen::common::Example;
use crate::objects::FieldKind;
use crate::CodeGenerator;
use crate::Object;
use crate::ObjectField;
use crate::ObjectKind;
use crate::Objects;
use crate::Reporter;
use camino::Utf8PathBuf;
use std::collections::BTreeSet;
use std::fmt::Write;

use super::common::get_documentation;
use super::common::ImageUrl;

macro_rules! putln {
    ($o:ident) => {let _ = writeln!($o);};
    ($o:ident, $($tt:tt)*) => {let _ = writeln!($o, $($tt)*);};
}

pub struct DocsCodeGenerator {
    docs_dir: Utf8PathBuf,
}

impl DocsCodeGenerator {
    pub fn new(docs_dir: impl Into<Utf8PathBuf>) -> Self {
        Self {
            docs_dir: docs_dir.into(),
        }
    }
}

impl CodeGenerator for DocsCodeGenerator {
    fn generate(
        &mut self,
        _reporter: &Reporter,
        objects: &Objects,
        _arrow_registry: &crate::ArrowRegistry,
    ) -> BTreeSet<camino::Utf8PathBuf> {
        re_tracing::profile_function!();

        let mut filepaths = BTreeSet::new();

        let components = objects.ordered_objects(Some(ObjectKind::Component));
        for object in objects.ordered_objects(Some(ObjectKind::Archetype)) {
            let title = object.snake_case_name();

            // skip test-only archetypes
            if title.starts_with("affix_fuzzer") {
                continue;
            }

            let order = object.order;
            let top_level_docs = get_documentation(&object.docs, &[]);
            let examples = object
                .docs
                .tagged_docs
                .get("example")
                .iter()
                .flat_map(|v| v.iter())
                .map(String::as_str)
                .map(Example::parse)
                .collect::<Vec<_>>();

            let mut o = String::new();

            frontmatter(&mut o, &title, order);
            putln!(o);
            for mut line in top_level_docs {
                if line.starts_with(char::is_whitespace) {
                    line.remove(0);
                }
                putln!(o, "{line}");
            }
            putln!(o);
            components_and_apis(&mut o, &components, &object.fields);
            putln!(o);
            example_list(&mut o, &examples);

            let path = self.docs_dir.join(format!("{title}.md"));
            super::common::write_file(&path, &o);
            filepaths.insert(path);
        }

        filepaths
    }
}

fn frontmatter(o: &mut String, title: &str, order: u32) {
    putln!(o, "---");
    putln!(o, "title: {title}");
    putln!(o, "order: {order}");
    putln!(o, "---");
}

fn components_and_apis(o: &mut String, components: &[&Object], fields: &[ObjectField]) {
    if fields.is_empty() {
        return;
    }

    putln!(o, "## Components and APIs");

    let required = fields
        .iter()
        .filter(|f| f.kind() == Some(FieldKind::Required))
        .filter_map(|f| find_component(f, components))
        .collect::<Vec<_>>();
    if !required.is_empty() {
        putln!(o);
        putln!(o, "Required:");
        for v in required {
            putln!(o, "* `{}`", v.snake_case_name());
        }
    }

    let recommended = fields
        .iter()
        .filter(|f| f.kind() == Some(FieldKind::Recommended))
        .filter_map(|f| find_component(f, components))
        .collect::<Vec<_>>();
    if !recommended.is_empty() {
        putln!(o);
        putln!(o, "Recommended:");
        for v in recommended {
            putln!(o, "* `{}`", v.snake_case_name());
        }
    }

    let optional = fields
        .iter()
        .filter(|f| f.kind() == Some(FieldKind::Optional))
        .filter_map(|f| find_component(f, components))
        .collect::<Vec<_>>();
    if !optional.is_empty() {
        putln!(o);
        putln!(o, "Optional:");
        for v in optional {
            putln!(o, "* `{}`", v.snake_case_name());
        }
    }
}

fn find_component<'a>(field: &ObjectField, components: &[&'a Object]) -> Option<&'a Object> {
    field
        .typ
        .fqname()
        .and_then(|fqname| components.iter().find(|c| c.fqname == fqname))
        .copied()
}

fn example_list(o: &mut String, examples: &[Example<'_>]) {
    if examples.is_empty() {
        return;
    }

    putln!(o, "## Examples");
    putln!(o);

    for Example { name, title, image } in examples {
        let title = title.unwrap_or(name);
        putln!(o, "### {title}");
        putln!(o);
        putln!(o, "code-example: {name}");
        putln!(o);

        image_url_stack(o, title, image.as_ref());
        putln!(o);
    }
}

fn image_url_stack(o: &mut String, title: &str, image_url: Option<&ImageUrl<'_>>) {
    let Some(image_url) = image_url else { return };

    match image_url {
        ImageUrl::Rerun(rerun) => {
            for line in rerun.image_stack(title) {
                putln!(o, "{line}");
            }
        }
        ImageUrl::Other(url) => {
            putln!(
                o,
                r#"<img src="{url}" alt="screenshot of {title} example">"#
            );
        }
    }
}
