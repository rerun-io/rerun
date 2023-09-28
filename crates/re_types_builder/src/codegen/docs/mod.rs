use crate::codegen::common::ExampleInfo;
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

macro_rules! putln {
    ($o:ident) => ( writeln!($o).ok() );
    ($o:ident, $($tt:tt)*) => ( writeln!($o, $($tt)*).ok() );
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
            // skip test-only archetypes
            if object.is_testing() {
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
                .map(ExampleInfo::parse)
                .collect::<Vec<_>>();

            let mut o = String::new();

            frontmatter(&mut o, &object.name, order);
            putln!(o);
            for mut line in top_level_docs {
                if line.starts_with(char::is_whitespace) {
                    line.remove(0);
                }
                putln!(o, "{line}");
            }
            putln!(o);
            write_archetype_fields(&mut o, &components, &object.fields);
            putln!(o);
            example_list(&mut o, &examples);

            let path = self
                .docs_dir
                .join(format!("{}.md", object.snake_case_name()));
            super::common::write_file(&path, &o);
            filepaths.insert(path);
        }

        filepaths
    }
}

fn frontmatter(o: &mut String, title: &str, order: u32) {
    putln!(o, "---");
    putln!(o, "title: \"{title}\"");
    putln!(o, "order: {order}");
    putln!(o, "---");
}

fn write_archetype_fields(o: &mut String, all_components: &[&Object], fields: &[ObjectField]) {
    if fields.is_empty() {
        return;
    }

    let (mut required, mut recommended, mut optional) = (Vec::new(), Vec::new(), Vec::new());
    for field in fields {
        let (target, component) = match field
            .kind()
            .and_then(|kind| Some((kind, find_component(field, all_components)?)))
        {
            Some((FieldKind::Required, component)) => (&mut required, component),
            Some((FieldKind::Recommended, component)) => (&mut recommended, component),
            Some((FieldKind::Optional, component)) => (&mut optional, component),
            _ => continue,
        };
        target.push(format!("`{}`", component.snake_case_name()));
    }

    putln!(o, "## Components");
    if !required.is_empty() {
        putln!(o);
        putln!(o, "**Required**: {}", required.join(", "));
    }
    if !recommended.is_empty() {
        putln!(o);
        putln!(o, "**Recommended**: {}", recommended.join(", "));
    }
    if !optional.is_empty() {
        putln!(o);
        putln!(o, "**Optional**: {}", optional.join(", "));
    }
}

fn find_component<'a>(field: &ObjectField, components: &[&'a Object]) -> Option<&'a Object> {
    field
        .typ
        .fqname()
        .and_then(|fqname| components.iter().find(|c| c.fqname == fqname))
        .copied()
}

fn example_list(o: &mut String, examples: &[ExampleInfo<'_>]) {
    if examples.is_empty() {
        return;
    }

    putln!(o, "## Examples");
    putln!(o);

    for ExampleInfo { name, title, image } in examples {
        let title = title.unwrap_or(name);
        putln!(o, "### {title}");
        putln!(o);
        putln!(o, "code-example: {name}");
        if let Some(image_url) = image {
            putln!(o);
            for line in image_url.image_stack() {
                putln!(o, "{line}");
            }
        }
        putln!(o);
    }
}
