//! Init method generation for Python codegen.

use itertools::{Itertools as _, chain};
use unindent::unindent;

use super::docs::quote_doc_lines;
use super::extension_class::ExtensionClass;
use super::typing::{quote_field_type_from_field, quote_parameter_type_alias};
use crate::codegen::Target;
use crate::{Object, ObjectField, ObjectKind, Objects, Reporter};

pub fn quote_init_parameter_from_field(
    field: &ObjectField,
    objects: &Objects,
    current_obj_fqname: &str,
) -> String {
    let type_annotation = if let Some(fqname) = field.typ.fqname() {
        quote_parameter_type_alias(fqname, current_obj_fqname, objects, field.typ.is_plural())
    } else {
        let type_annotation = quote_field_type_from_field(objects, field, false).0;
        // Relax type annotation for numpy arrays.
        if type_annotation.starts_with("npt.NDArray") {
            "npt.ArrayLike".to_owned()
        } else {
            type_annotation
        }
    };

    if field.is_nullable {
        format!("{}: {} | None = None", field.name, type_annotation)
    } else {
        format!("{}: {}", field.name, type_annotation)
    }
}

pub fn compute_init_parameters(obj: &Object, objects: &Objects) -> Vec<String> {
    // If the type is fully transparent (single non-nullable field and not an archetype),
    // we have to use the "{obj.name}Like" type directly since the type of the field itself might be too narrow.
    // -> Whatever type aliases there are for this type, we need to pick them up.
    if obj.kind != ObjectKind::Archetype
        && let [single_field] = obj.fields.as_slice()
        && !single_field.is_nullable
    {
        vec![format!(
            "{}: {}",
            single_field.name,
            quote_parameter_type_alias(&obj.fqname, &obj.fqname, objects, false)
        )]
    } else if obj.is_union() {
        vec![format!(
            "inner: {} | None = None",
            quote_parameter_type_alias(&obj.fqname, &obj.fqname, objects, false)
        )]
    } else {
        let required = obj
            .fields
            .iter()
            .filter(|field| !field.is_nullable)
            .map(|field| quote_init_parameter_from_field(field, objects, &obj.fqname))
            .collect_vec();

        let optional = obj
            .fields
            .iter()
            .filter(|field| field.is_nullable)
            .map(|field| quote_init_parameter_from_field(field, objects, &obj.fqname))
            .collect_vec();

        if 2 < required.len() {
            // There's a lot of required arguments.
            // Using positional arguments would make usage hard to read.
            // better for force kw-args for ALL arguments:
            chain!(std::iter::once("*".to_owned()), required, optional).collect()
        } else if optional.is_empty() {
            required
        } else if obj.name == "AnnotationInfo" {
            // TODO(#6836): rewrite AnnotationContext
            chain!(required, optional).collect()
        } else {
            // Force kw-args for all optional arguments:
            chain!(required, std::iter::once("*".to_owned()), optional).collect()
        }
    }
}

pub fn compute_init_parameter_docs(
    reporter: &Reporter,
    obj: &Object,
    objects: &Objects,
) -> Vec<String> {
    if obj.is_union() {
        Vec::new()
    } else {
        obj.fields
            .iter()
            .filter_map(|field| {
                let doc_content = field.docs.lines_for(reporter, objects, Target::Python);
                if doc_content.is_empty() {
                    if !field.is_testing() && obj.fields.len() > 1 {
                        reporter.error(
                            &field.virtpath,
                            &field.fqname,
                            format!("Field {} is missing documentation", field.name),
                        );
                    }
                    None
                } else {
                    Some(format!(
                        "{}:\n    {}",
                        field.name,
                        doc_content.join("\n    ")
                    ))
                }
            })
            .collect::<Vec<_>>()
    }
}

pub fn quote_init_method(
    reporter: &Reporter,
    obj: &Object,
    ext_class: &ExtensionClass,
    objects: &Objects,
) -> String {
    let head = format!(
        "def __init__(self: Any, {}) -> None:",
        compute_init_parameters(obj, objects).join(", ")
    );

    let parameter_docs = compute_init_parameter_docs(reporter, obj, objects);
    let mut doc_string_lines = vec![format!(
        "Create a new instance of the {} {}.",
        obj.name,
        obj.kind.singular_name().to_lowercase()
    )];
    if !parameter_docs.is_empty() {
        doc_string_lines.push("\n".to_owned());
        doc_string_lines.push("Parameters".to_owned());
        doc_string_lines.push("----------".to_owned());
        for doc in parameter_docs {
            doc_string_lines.push(doc);
        }
    }
    let doc_block = quote_doc_lines(doc_string_lines);

    let custom_init_hint = format!(
        "# You can define your own __init__ function as a member of {} in {}",
        ext_class.name, ext_class.file_name
    );

    let forwarding_call = if obj.is_union() {
        "self.inner = inner".to_owned()
    } else {
        let attribute_init = obj
            .fields
            .iter()
            .map(|field| format!("{}={}", field.name, field.name))
            .collect::<Vec<_>>();

        format!("self.__attrs_init__({})", attribute_init.join(", "))
    };

    // Make sure Archetypes catch and log exceptions as a fallback
    let forwarding_call = if obj.kind == ObjectKind::Archetype {
        unindent(&format!(
            r#"
            with catch_and_log_exceptions(context=self.__class__.__name__):
                {forwarding_call}
                return
            self.__attrs_clear__()
            "#
        ))
    } else {
        forwarding_call
    };

    format!(
        "{head}\n{}",
        indent::indent_all_by(
            4,
            format!("{doc_block}{custom_init_hint}\n{forwarding_call}"),
        )
    )
}
