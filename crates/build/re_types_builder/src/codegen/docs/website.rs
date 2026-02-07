//! Generate the markdown files shown at <https://rerun.io/docs/reference/types>.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use camino::Utf8PathBuf;
use itertools::Itertools as _;

use crate::codegen::common::ExampleInfo;
use crate::codegen::{Target, autogen_warning};
use crate::objects::{FieldKind, ViewReference};
use crate::{
    CodeGenerator, GeneratedFiles, Object, ObjectField, ObjectKind, Objects, Reporter, Type,
};

pub const DATAFRAME_VIEW_FQNAME: &str = "rerun.blueprint.views.DataframeView";

/// Like [`writeln!`], but without a [`Result`].
macro_rules! putln {
    ($o:ident) => ( { writeln!($o).ok(); } );
    ($o:ident, $($tt:tt)*) => ( { writeln!($o, $($tt)*).unwrap(); } );
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

type ViewsPerArchetype = BTreeMap<String, Vec<ViewReference>>;

impl CodeGenerator for DocsCodeGenerator {
    fn generate(
        &mut self,
        reporter: &Reporter,
        objects: &Objects,
        type_registry: &crate::TypeRegistry,
    ) -> GeneratedFiles {
        re_tracing::profile_function!();

        let mut files_to_write = GeneratedFiles::default();

        // Gather view type mapping per object.
        let views_per_archetype = collect_view_types_per_archetype(objects);

        let (mut archetypes, mut components, mut datatypes, mut views) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());

        for object in objects.values() {
            // skip test-only archetypes
            if object.is_testing() {
                continue;
            }

            // Skip blueprint stuff, too early
            if object.scope() == Some("blueprint".to_owned()) && object.kind != ObjectKind::View {
                continue;
            }

            match object.kind {
                ObjectKind::Datatype => datatypes.push(object),
                ObjectKind::Component => components.push(object),
                ObjectKind::Archetype => archetypes.push(object),
                ObjectKind::View => views.push(object),
            }

            let page = object_page(
                reporter,
                objects,
                object,
                type_registry,
                &views_per_archetype,
            );
            let path = self.docs_dir.join(format!(
                "{}/{}.md",
                object.kind.plural_snake_case(),
                object.snake_case_name()
            ));
            files_to_write.insert(path, page);
        }

        for (kind, order, prelude, kind_objects) in [
            (
                ObjectKind::Archetype,
                1,
                r"Archetypes are bundles of components for which the Rerun viewer has first-class
built-in support. See [Entities and Components](../../concepts/logging-and-ingestion/entity-component.md) and
[Visualizers and Overrides](../../concepts/visualization/visualizers-and-overrides.md) for more information.

This page lists all built-in archetypes.",
                &archetypes,
            ),
            (
                ObjectKind::Component,
                2,
                r"Components are the fundamental unit of logging in Rerun. This page lists all built-in components.

An entity can only ever contain a single array of any given component type.
If you log the same component several times on an entity, the last value (or array of values) will overwrite the previous.

For more information on the relationship between **archetypes** and **components**, check out the concept page
on [Entities and Components](../../concepts/logging-and-ingestion/entity-component.md).",
                &components,
            ),
            (
                ObjectKind::Datatype,
                3,
                r"Data types are the lowest layer of the data model hierarchy. They are re-usable types used by the components.",
                &datatypes,
            ),
            (
                ObjectKind::View,
                4,
                r"Views are the panels shown in the viewer's viewport and the primary means of inspecting & visualizing previously logged data. This page lists all built-in views.",
                &views,
            ),
        ] {
            let page = index_page(reporter, objects, kind, order, prelude, kind_objects);
            let path = self
                .docs_dir
                .join(format!("{}.md", kind.plural_snake_case()));
            files_to_write.insert(path, page);
        }

        files_to_write
    }
}

fn collect_view_types_per_archetype(objects: &Objects) -> ViewsPerArchetype {
    let mut view_types_per_object = ViewsPerArchetype::new();
    for object in objects.objects.values() {
        if let Some(view_types) = object.archetype_view_types() {
            view_types_per_object.insert(object.fqname.clone(), view_types);
        }
    }

    view_types_per_object
}

fn index_page(
    reporter: &Reporter,
    all_objects: &Objects,
    kind: ObjectKind,
    order: u64,
    prelude: &str,
    objects: &[&Object],
) -> String {
    let mut page = String::new();

    write_frontmatter(&mut page, kind.plural_name(), Some(order));
    putln!(page);
    putln!(page, "{prelude}");
    putln!(page);

    let mut any_category = false;
    for (category, objects) in &objects
        .iter()
        .sorted_by(|a, b| {
            // Put other category last.
            if a.doc_category().is_none() {
                std::cmp::Ordering::Greater
            } else if b.doc_category().is_none() {
                std::cmp::Ordering::Less
            } else {
                a.doc_category().cmp(&b.doc_category())
            }
        })
        .chunk_by(|o| o.doc_category())
    {
        if category.is_some() {
            any_category = true;
        }
        if let Some(category) = category.or_else(|| {
            if any_category {
                Some("Other".to_owned())
            } else {
                None
            }
        }) {
            putln!(page, "## {category}");
        }
        putln!(page);

        for object in objects.sorted_by_key(|object| &object.name) {
            let deprecation_note = if object.is_deprecated() {
                "⚠️ _deprecated_ "
            } else {
                ""
            };

            putln!(
                page,
                "* {deprecation_note}[`{}`]({}/{}.md): {}",
                object.name,
                object.kind.plural_snake_case(),
                object.snake_case_name(),
                object
                    .docs
                    .first_line(reporter, all_objects, Target::WebDocsMarkdown)
                    .unwrap_or_default(),
            );
        }
        putln!(page);
    }

    page
}

fn object_page(
    reporter: &Reporter,
    objects: &Objects,
    object: &Object,
    type_registry: &crate::TypeRegistry,
    views_per_archetype: &ViewsPerArchetype,
) -> String {
    let top_level_docs = object
        .docs
        .lines_for(reporter, objects, Target::WebDocsMarkdown);

    if top_level_docs.is_empty() {
        reporter.error(&object.virtpath, &object.fqname, "Undocumented object");
    }

    let examples = &object.docs.only_lines_tagged("example");
    let examples = examples
        .iter()
        .map(|line| ExampleInfo::parse(line))
        .collect::<Vec<_>>();

    let mut page = String::new();

    let title = if object.is_deprecated() {
        format!("{} (deprecated)", object.name)
    } else {
        object.name.clone()
    };

    write_frontmatter(&mut page, &title, None);
    putln!(page);

    if let Some(docline_summary) = object.state.docline_summary() {
        page.push_str(&docline_summary);
        putln!(page);
    }

    for line in top_level_docs {
        putln!(page, "{line}");
    }
    putln!(page);

    match object.kind {
        ObjectKind::Datatype | ObjectKind::Component => {
            write_fields(reporter, objects, &mut page, object);
        }
        ObjectKind::Archetype => {
            write_archetype_fields(objects, &mut page, object, views_per_archetype);
        }
        ObjectKind::View => {
            write_view_properties(reporter, objects, &mut page, object);
        }
    }

    if matches!(object.kind, ObjectKind::Datatype | ObjectKind::Component) {
        let datatype = &type_registry.get(&object.fqname);
        putln!(page);
        putln!(page, "## Arrow datatype");
        putln!(page, "```");
        super::datatype_docs(&mut page, datatype);
        putln!(page);
        putln!(page, "```");
    }

    putln!(page);
    putln!(page, "## API reference links");
    list_links(&mut page, object);

    putln!(page);
    write_example_list(&mut page, &examples);

    match object.kind {
        ObjectKind::Datatype | ObjectKind::Component => {
            putln!(page);
            write_used_by(&mut page, reporter, objects, object);
        }
        ObjectKind::Archetype => {
            if examples.is_empty() {
                if object.virtpath.starts_with("//testing") {
                    // do nothing
                } else if object.virtpath.starts_with("//archetypes") {
                    // actual public archetypes: hard error
                    reporter.error(&object.virtpath, &object.fqname, "No examples");
                } else {
                    // everything else (including experimental blueprint stuff): simple warning
                    reporter.warn(&object.virtpath, &object.fqname, "No examples");
                }
            }
        }
        ObjectKind::View => {
            putln!(page);
            write_visualized_archetypes(reporter, objects, &mut page, object, views_per_archetype);
        }
    }

    page
}

fn list_links(page: &mut String, object: &Object) {
    let speculative_marker = if object.is_attr_set(crate::ATTR_DOCS_UNRELEASED) {
        "?speculative-link"
    } else {
        ""
    };

    if object.kind == ObjectKind::View {
        // More complicated link due to scope
        putln!(
            page,
            " * 🐍 [Python API docs for `{}`](https://ref.rerun.io/docs/python/stable/common/{}_{}{}#rerun.{}.{}.{})",
            object.name,
            object.scope().unwrap_or_default(),
            object.kind.plural_snake_case(),
            speculative_marker,
            object.scope().unwrap_or_default(),
            object.kind.plural_snake_case(),
            object.name
        );
    } else {
        let cpp_link = if object.is_enum() {
            // Can't link to enums directly 🤷
            format!(
                "https://ref.rerun.io/docs/cpp/stable/namespacererun_1_1{}.html",
                object.kind.plural_snake_case()
            )
        } else {
            // `_1` is doxygen's replacement for ':'
            // https://github.com/doxygen/doxygen/blob/Release_1_9_8/src/util.cpp#L3532
            format!(
                "https://ref.rerun.io/docs/cpp/stable/structrerun_1_1{}_1_1{}.html",
                object.kind.plural_snake_case(),
                object.name
            )
        };

        // In alphabetical order by language.
        putln!(
            page,
            " * 🌊 [C++ API docs for `{}`]({cpp_link}{speculative_marker})",
            object.name,
        );

        putln!(
            page,
            " * 🐍 [Python API docs for `{}`](https://ref.rerun.io/docs/python/stable/common/{}{}#rerun.{}.{})",
            object.name,
            object.module_name().replace('/', "_"), // E.g. `blueprint_archetypes`
            speculative_marker,
            object.module_name().replace('/', "."), // E.g. `blueprint.archetypes`
            object.name
        );

        putln!(
            page,
            " * 🦀 [Rust API docs for `{}`](https://docs.rs/rerun/latest/rerun/{}/{}.{}.html{speculative_marker})",
            object.name,
            object.kind.plural_snake_case(),
            if object.is_struct() { "struct" } else { "enum" },
            object.name,
        );
    }
}

fn write_frontmatter(o: &mut String, title: &str, order: Option<u64>) {
    putln!(o, "---");
    putln!(o, "title: {title:?}");
    if let Some(order) = order {
        // The order is used to sort `rerun.io/docs` side navigation
        putln!(o, "order: {order}");
    }
    putln!(o, "---");
    // Can't put the autogen warning before the frontmatter, stuff breaks down then.
    putln!(o, "<!-- {} -->", autogen_warning!());
}

fn write_fields(reporter: &Reporter, objects: &Objects, o: &mut String, object: &Object) {
    if object.fields.is_empty() {
        return;
    }

    fn type_info(objects: &Objects, ty: &Type) -> String {
        fn atomic(name: &str) -> String {
            format!("`{name}`")
        }

        match ty {
            Type::Unit => unreachable!("Should be handled elsewhere"),

            // We use explicit, arrow-like names:
            Type::UInt8 => atomic("uint8"),
            Type::UInt16 => atomic("uint16"),
            Type::UInt32 => atomic("uint32"),
            Type::UInt64 => atomic("uint64"),
            Type::Int8 => atomic("int8"),
            Type::Int16 => atomic("int16"),
            Type::Int32 => atomic("int32"),
            Type::Int64 => atomic("int64"),
            Type::Bool => atomic("boolean"),
            Type::Float16 => atomic("float16"),
            Type::Float32 => atomic("float32"),
            Type::Float64 => atomic("float64"),
            Type::Binary => atomic("binary"),
            Type::String => atomic("utf8"),

            Type::Array { elem_type, length } => {
                format!(
                    "{length}x {}",
                    type_info(objects, &Type::from(elem_type.clone()))
                )
            }
            Type::Vector { elem_type } => {
                format!(
                    "List of {}",
                    type_info(objects, &Type::from(elem_type.clone()))
                )
            }
            Type::Object { fqname } => {
                let ty = objects.get(fqname).unwrap();
                format!(
                    "[`{}`](../{}/{}.md)",
                    ty.name,
                    ty.kind.plural_snake_case(),
                    ty.snake_case_name()
                )
            }
        }
    }

    if object.is_arrow_transparent() {
        assert!(object.is_struct());
        assert_eq!(object.fields.len(), 1);
        let field_type = &object.fields[0].typ;
        if object.kind == ObjectKind::Component && matches!(field_type, Type::Object { .. }) {
            putln!(o, "## Rerun datatype");
            putln!(o, "{}", type_info(objects, field_type));
            putln!(o);
        } else {
            // The arrow datatype section covers it
        }
        return; // This is just a wrapper type, so don't show the "Fields" section
    }

    let mut fields = Vec::new();
    for field in &object.fields {
        let mut field_string = format!("#### `{}`", field.name);

        if let Some(enum_or_union_variant_value) = field.enum_or_union_variant_value {
            if let Some(enum_integer_type) = object.enum_integer_type() {
                field_string.push_str(&format!(
                    " = {}",
                    enum_integer_type.format_value(enum_or_union_variant_value)
                ));
            } else {
                field_string.push_str(&format!(" = {enum_or_union_variant_value}"));
            }
        }
        field_string.push('\n');

        if !object.is_enum() {
            field_string.push_str("Type: ");
            if field.typ == Type::Unit {
                field_string.push_str("`null`");
            } else {
                if field.is_nullable {
                    field_string.push_str("nullable ");
                }
                field_string.push_str(&type_info(objects, &field.typ));
            }
            field_string.push('\n');
            field_string.push('\n');
        }

        for line in field
            .docs
            .lines_for(reporter, objects, Target::WebDocsMarkdown)
        {
            field_string.push_str(&line);
            field_string.push('\n');
        }

        fields.push(field_string);
    }

    if !fields.is_empty() {
        let heading = match object.class {
            crate::ObjectClass::Struct => "## Fields",
            crate::ObjectClass::Enum(_) | crate::ObjectClass::Union => "## Variants",
        };
        putln!(o, "{heading}");
        for field in fields {
            putln!(o, "{field}");
        }
    }
}

fn write_used_by(o: &mut String, reporter: &Reporter, objects: &Objects, object: &Object) {
    let mut used_by = Vec::new();
    for ty in objects.values() {
        // Since blueprints are being skipped there used-by links should also be skipped
        if ty.scope() == Some("blueprint".to_owned()) {
            continue;
        }
        for field in &ty.fields {
            if field.typ.fqname() == Some(object.fqname.as_str()) {
                let is_unreleased = ty.is_attr_set(crate::ATTR_DOCS_UNRELEASED);
                let speculative_marker = if is_unreleased {
                    "?speculative-link"
                } else {
                    ""
                };
                used_by.push(format!(
                    "* [`{}`](../{}/{}.md{})",
                    ty.name,
                    ty.kind.plural_snake_case(),
                    ty.snake_case_name(),
                    speculative_marker
                ));
            }
        }
    }
    used_by.sort();
    used_by.dedup(); // The same datatype can be used multiple times by the same component

    if used_by.is_empty() {
        // NOTE: there are some false positives here, because unions can only
        // reference other tables, but they are unwrapped in the codegen.
        // So for instance: `union Angle` uses `rerun.datatypes.Float32` in
        // `angle.fbs`, but in the generated code that datatype is unused.
        if false {
            reporter.warn(&object.virtpath, &object.fqname, "Unused object");
        }
    } else {
        putln!(o, "## Used by");
        putln!(o);
        for ty in used_by {
            putln!(o, "{ty}");
        }
    }
}

fn write_archetype_fields(
    objects: &Objects,
    page: &mut String,
    object: &Object,
    view_per_archetype: &ViewsPerArchetype,
) {
    if object.fields.is_empty() {
        return;
    }

    putln!(page, "## Fields");
    let grouped_by_kind: HashMap<FieldKind, Vec<&ObjectField>> =
        object.fields.iter().into_group_map_by(|field| {
            field
                .kind()
                .expect("All archetype fields must have a 'kind'")
        });

    for kind in FieldKind::ALL {
        let Some(fields) = grouped_by_kind.get(&kind) else {
            continue;
        };

        putln!(page, "### {kind}");

        for field in fields {
            let Some(fqname) = field.typ.fqname() else {
                panic!("Archetype field should be object: {:?}", field.name);
            };
            let Some(ty) = objects.get(fqname) else {
                panic!("Archetype field should be object: {:?}", field.name);
            };

            putln!(
                page,
                "* `{}`: [`{}`](../{}/{}.md)",
                field.name,
                ty.name,
                ty.kind.plural_snake_case(),
                ty.snake_case_name(),
            );
        }

        putln!(page);
    }

    putln!(page);
    putln!(page, "## Can be shown in");

    if let Some(view_types) = view_per_archetype.get(&object.fqname) {
        for ViewReference {
            view_name,
            explanation,
        } in view_types
        {
            page.push_str(&format!(
                "* [{view_name}](../views/{}.md)",
                re_case::to_snake_case(view_name)
            ));
            if let Some(explanation) = explanation {
                page.push_str(&format!(" ({explanation})"));
            }
            putln!(page);
        }
    }

    // Special case for dataframe view: it can display anything.
    putln!(page, "* [DataframeView](../views/dataframe_view.md)");
}

fn write_visualized_archetypes(
    reporter: &Reporter,
    objects: &Objects,
    page: &mut String,
    view: &Object,
    views_per_archetype: &ViewsPerArchetype,
) {
    let mut archetype_fqnames = Vec::new();
    for (fqname, reference) in views_per_archetype {
        for ViewReference {
            view_name,
            explanation,
        } in reference
        {
            if view_name == &view.name {
                archetype_fqnames.push((fqname.clone(), explanation));
            }
        }
    }

    if archetype_fqnames.is_empty() && view.fqname != DATAFRAME_VIEW_FQNAME {
        reporter.error(&view.virtpath, &view.fqname, "No archetypes use this view.");
        return;
    }

    // Put the archetypes in alphabetical order but put the ones with extra explanation last.
    archetype_fqnames.sort_by_key(|(fqname, explanation)| (explanation.is_some(), fqname.clone()));

    putln!(page, "## Visualized archetypes");
    putln!(page);

    // special case for dataframe view
    if view.fqname == DATAFRAME_VIEW_FQNAME {
        putln!(page, "Any data can be displayed by the Dataframe view.");
    } else {
        for (fqname, explanation) in archetype_fqnames {
            let object = &objects[&fqname];
            page.push_str(&format!(
                "* [`{}`](../{}/{}.md)",
                object.name,
                object.kind.plural_snake_case(),
                object.snake_case_name()
            ));
            if let Some(explanation) = explanation {
                page.push_str(&format!(" ({explanation})"));
            }
            putln!(page);
        }
    }
    putln!(page);
}

fn write_view_properties(reporter: &Reporter, objects: &Objects, page: &mut String, view: &Object) {
    if view.fields.is_empty() {
        return;
    }

    putln!(page, "## Properties");
    putln!(page);

    // Each field in a view should be a property
    for field in &view.fields {
        write_view_property(reporter, objects, page, field);
    }
}

fn write_view_property(
    reporter: &Reporter,
    objects: &Objects,
    o: &mut String,
    field: &ObjectField,
) {
    putln!(o, "### `{}`", field.name);

    let top_level_docs = field
        .docs
        .lines_for(reporter, objects, Target::WebDocsMarkdown);

    if top_level_docs.is_empty() {
        reporter.error(&field.virtpath, &field.fqname, "Undocumented view property");
    }

    for line in top_level_docs {
        putln!(o, "{line}");
    }

    // If there's more than one fields on this type, list them:
    let Some(field_fqname) = field.typ.fqname() else {
        return;
    };
    let object = &objects[field_fqname];

    let mut fields = Vec::new();
    for field in &object.fields {
        fields.push(format!(
            "* `{}`: {}",
            field.name,
            field
                .docs
                .first_line(reporter, objects, Target::WebDocsMarkdown)
                .unwrap_or_default()
        ));
    }

    if fields.len() > 1 {
        putln!(o);
        for field in fields {
            putln!(o, "{field}");
        }
    }

    // Note that we don't list links to reference docs for this type since this causes a lot of clutter.
}

fn write_example_list(o: &mut String, examples: &[ExampleInfo<'_>]) {
    if examples.is_empty() {
        return;
    }

    if examples.len() > 1 {
        putln!(o, "## Examples");
    } else {
        putln!(o, "## Example");
    }
    putln!(o);

    for ExampleInfo {
        path,
        name,
        title,
        image,
        exclude_from_api_docs: _,
        missing_extensions: _,
    } in examples
    {
        let title = title.unwrap_or(name);
        putln!(o, "### {title}");
        putln!(o);
        putln!(o, "snippet: {path}");
        if let Some(image_url) = image {
            putln!(o);
            for line in image_url.image_stack().snippet_id(path).finish() {
                putln!(o, "{line}");
            }
        }
        putln!(o);
    }
}
