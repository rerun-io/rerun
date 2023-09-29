use crate::codegen::common::ExampleInfo;
use crate::objects::FieldKind;
use crate::CodeGenerator;
use crate::Object;
use crate::ObjectKind;
use crate::Objects;
use crate::Reporter;
use camino::Utf8PathBuf;
use std::collections::BTreeSet;
use std::fmt::Write;

type ObjectMap = std::collections::BTreeMap<String, Object>;

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

        let object_map = &objects.objects;
        for object in objects.ordered_objects(None) {
            // skip test-only archetypes
            if object.is_testing() {
                continue;
            }

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

            frontmatter(&mut o, &object.name);
            putln!(o);
            for mut line in top_level_docs {
                if line.starts_with(char::is_whitespace) {
                    line.remove(0);
                }
                putln!(o, "{line}");
            }
            putln!(o);

            match object.kind {
                ObjectKind::Datatype | ObjectKind::Component => {
                    write_fields(&mut o, object, object_map);
                }
                ObjectKind::Archetype => write_archetype_fields(&mut o, object, object_map),
            }

            putln!(o);
            write_example_list(&mut o, &examples);

            match object.kind {
                ObjectKind::Datatype | ObjectKind::Component => {
                    putln!(o);
                    write_used_by(&mut o, object, object_map);
                }
                ObjectKind::Archetype => {}
            }

            let kind_dir = match object.kind {
                ObjectKind::Datatype => "datatypes",
                ObjectKind::Component => "components",
                ObjectKind::Archetype => "archetypes",
            };
            let path = self
                .docs_dir
                .join(format!("{kind_dir}/{}.md", object.snake_case_name()));
            super::common::write_file(&path, &o);
            filepaths.insert(path);
        }

        filepaths
    }
}

fn frontmatter(o: &mut String, title: &str) {
    putln!(o, "---");
    putln!(o, "title: {title:?}");
    putln!(o, "---");
}

fn write_fields(o: &mut String, object: &Object, object_map: &ObjectMap) {
    if object.fields.is_empty() {
        return;
    }

    let mut fields = Vec::new();
    for field in &object.fields {
        let Some(fqname) = field.typ.fqname() else {
            continue;
        };
        let Some(ty) = object_map.get(fqname) else {
            continue;
        };
        fields.push(format!(
            "* {}: [`{}`](../{}/{}.md)",
            field.name,
            ty.name,
            ty.kind.dirname(),
            ty.snake_case_name()
        ));
    }

    if !fields.is_empty() {
        putln!(o, "## Fields");
        putln!(o);
    }
    for field in fields {
        putln!(o, "{field}");
    }
}

fn write_used_by(o: &mut String, object: &Object, object_map: &ObjectMap) {
    let mut used_by = Vec::new();
    for ty in object_map.values() {
        for field in &ty.fields {
            if field.typ.fqname() == Some(object.fqname.as_str()) {
                used_by.push(format!(
                    "* [`{}`](../{}/{}.md)",
                    ty.name,
                    ty.kind.dirname(),
                    ty.snake_case_name()
                ));
            }
        }
    }

    if !used_by.is_empty() {
        putln!(o, "## Related");
        putln!(o);
    }
    for ty in used_by {
        putln!(o, "{ty}");
    }
}

fn write_archetype_fields(o: &mut String, object: &Object, object_map: &ObjectMap) {
    if object.fields.is_empty() {
        return;
    }

    // collect names of field _components_ by their `FieldKind`
    let (mut required, mut recommended, mut optional) = (Vec::new(), Vec::new(), Vec::new());
    for field in &object.fields {
        let Some(fqname) = field.typ.fqname() else {
            continue;
        };
        let Some(ty) = object_map.get(fqname) else {
            continue;
        };
        let target = match field.kind() {
            Some(FieldKind::Required) => &mut required,
            Some(FieldKind::Recommended) => &mut recommended,
            Some(FieldKind::Optional) => &mut optional,
            _ => continue,
        };
        target.push(format!(
            "[`{}`](../{}/{}.md)",
            ty.name,
            ty.kind.dirname(),
            ty.snake_case_name()
        ));
    }

    if !required.is_empty() || !recommended.is_empty() || !optional.is_empty() {
        putln!(o, "## Components");
    }
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

fn write_example_list(o: &mut String, examples: &[ExampleInfo<'_>]) {
    if examples.is_empty() {
        return;
    }

    if examples.len() > 1 {
        putln!(o, "## Examples");
    } else {
        putln!(o, "## Example");
    };
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

trait ObjectKindExt {
    fn dirname(&self) -> &'static str;
}

impl ObjectKindExt for ObjectKind {
    fn dirname(&self) -> &'static str {
        match self {
            ObjectKind::Datatype => "datatypes",
            ObjectKind::Component => "components",
            ObjectKind::Archetype => "archetypes",
        }
    }
}
