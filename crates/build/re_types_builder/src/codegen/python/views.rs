use crate::{
    codegen::{
        common::StringExt as _,
        python::{quote_doc_lines, quote_obj_docs},
    },
    Object, Objects, Reporter, ATTR_PYTHON_ALIASES, ATTR_RERUN_VIEW_IDENTIFIER,
};

pub fn code_for_view(reporter: &Reporter, objects: &Objects, obj: &Object) -> String {
    assert!(obj.is_struct());

    let mut code = String::new();

    code.push_indented(
        0,
        "
from .. import archetypes as blueprint_archetypes
from .. import components as blueprint_components
from ... import datatypes
from ... import components
from ..._baseclasses import AsComponents, ComponentBatchLike
from ...datatypes import EntityPathLike, Utf8Like
from ..api import SpaceView, SpaceViewContentsLike
",
        1,
    );
    code.push('\n');

    code.push_indented(0, format!("class {}(SpaceView):", obj.name), 1);
    code.push_indented(1, quote_obj_docs(obj), 1);

    code.push_indented(1, init_method(reporter, objects, obj), 1);

    code
}

fn init_method(reporter: &Reporter, objects: &Objects, obj: &Object) -> String {
    let mut code = r#"def __init__(
    self, *,
    origin: EntityPathLike = "/",
    contents: SpaceViewContentsLike = "$origin/**",
    name: Utf8Like | None = None,
    visible: datatypes.BoolLike | None = None,
    defaults: list[Union[AsComponents, ComponentBatchLike]] = [],
    overrides: dict[EntityPathLike, list[ComponentBatchLike]] = {},
    "#
    .to_owned();

    for property in &obj.fields {
        let Some(property_type_fqname) = property.typ.fqname() else {
            reporter.error(
                &obj.virtpath,
                &property.fqname,
                "View properties must be archetypes.",
            );
            continue;
        };
        let property_type = &objects[property_type_fqname];
        let property_type_name = &property_type.name;

        // Right now we don't create "<ArchetypeName>Like" type aliases for archetypes.
        // So we have to list all the possible types here.
        // For archetypes in general this would only be confusing, but for Space View properties it
        // could be useful to make the annotation here shorter.
        let additional_type_annotations = property_type
            .try_get_attr::<String>(ATTR_PYTHON_ALIASES)
            .map_or(String::new(), |aliases| {
                let mut types = String::new();
                for alias in aliases.split(',') {
                    types.push_str(alias.trim());
                    types.push_str(" | ");
                }
                types
            });

        let parameter_name = &property.name;
        code.push_str(&format!(
            "{parameter_name}: blueprint_archetypes.{property_type_name} | {additional_type_annotations} None = None,\n"
        ));
    }

    code.push_indented(1, ") -> None:", 1);

    let mut init_docs = Vec::new();
    init_docs.push(format!(
        "Construct a blueprint for a new {} view.",
        obj.name
    ));
    init_docs.push(String::new());
    init_docs.push("Parameters".to_owned());
    init_docs.push("----------".to_owned());
    let mut parameter_docs = vec![
        (
            "origin",
            "The `EntityPath` to use as the origin of this view.
All other entities will be transformed to be displayed relative to this origin."
                .to_owned(),
        ),
        (
            "contents",
            "The contents of the view specified as a query expression.
This is either a single expression, or a list of multiple expressions.
See [rerun.blueprint.archetypes.SpaceViewContents][]."
                .to_owned(),
        ),
        ("name", "The display name of the view.".to_owned()),
        (
            "visible",
            "Whether this view is visible.

Defaults to true if not specified."
                .to_owned(),
        ),
        (
            "defaults",
            "List of default components or component batches to add to the space view. When an archetype
in the view is missing a component included in this set, the value of default will be used
instead of the normal fallback for the visualizer.".to_owned(),
        ),
        (
            "overrides",
            "Dictionary of overrides to apply to the space view. The key is the path to the entity where the override
should be applied. The value is a list of component or component batches to apply to the entity.

Important note: the path must be a fully qualified entity path starting at the root. The override paths
do not yet support `$origin` relative paths or glob expressions.
This will be addressed in: [https://github.com/rerun-io/rerun/issues/6673][].".to_owned(),)
    ];
    for field in &obj.fields {
        let doc_content = field.docs.doc_lines_for_untagged_and("py");
        if doc_content.is_empty() {
            reporter.error(
                &field.virtpath,
                &field.fqname,
                format!("Field {} is missing documentation", field.name),
            );
        }

        parameter_docs.push((&field.name, doc_content.join("\n")));
    }

    for (name, doc) in parameter_docs {
        let mut doc_string = format!("{name}:\n");
        doc_string.push_indented(1, doc, 0);
        init_docs.push(doc_string);
    }
    code.push_indented(1, quote_doc_lines(init_docs), 1);

    let Some(identifier): Option<String> = obj.try_get_attr(ATTR_RERUN_VIEW_IDENTIFIER) else {
        reporter.error(
            &obj.virtpath,
            &obj.fqname,
            format!("Missing {ATTR_RERUN_VIEW_IDENTIFIER} attribute for view"),
        );
        return code;
    };

    code.push_indented(1, "properties: dict[str, AsComponents] = {}", 1);

    for property in &obj.fields {
        let Some(property_type_fqname) = property.typ.fqname() else {
            reporter.error(
                &obj.virtpath,
                &property.fqname,
                "View properties must be archetypes.",
            );
            continue;
        };

        let parameter_name = &property.name;
        let property_type = &objects[property_type_fqname];
        let property_name = &property_type.name;
        let property_type_name = format!("blueprint_archetypes.{}", &property_type.name);
        code.push_indented(1, &format!("if {parameter_name} is not None:"), 1);
        code.push_indented(
            2,
            &format!("if not isinstance({parameter_name}, {property_type_name}):"),
            1,
        );
        code.push_indented(
            3,
            &format!("{parameter_name} = {property_type_name}({parameter_name})"),
            1,
        );
        code.push_indented(
            2,
            &format!(r#"properties["{property_name}"] = {parameter_name}"#),
            2,
        );
    }
    code.push_indented(
        1,
        &format!(r#"super().__init__(class_identifier="{identifier}", origin=origin, contents=contents, name=name, visible=visible, properties=properties, defaults=defaults, overrides=overrides)"#),
        1,
    );

    code
}
