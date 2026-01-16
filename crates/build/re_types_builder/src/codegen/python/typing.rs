//! Type-related helpers for Python codegen.

use unindent::unindent;

use super::extension_class::ExtensionClasses;
use super::object_ext::PythonObjectExt;
use crate::codegen::StringExt as _;
use crate::{
    ATTR_PYTHON_ALIASES, ATTR_PYTHON_ARRAY_ALIASES, ElementType, Object, ObjectField, ObjectKind,
    Objects, Type,
};

/// Returns type name as string and whether it was force unwrapped.
///
/// Specifying `unwrap = true` will unwrap the final type before returning it, e.g. `Vec<String>`
/// becomes just `String`.
/// The returned boolean indicates whether there was anything to unwrap at all.
pub fn quote_field_type_from_field(
    _objects: &Objects,
    field: &ObjectField,
    unwrap: bool,
) -> (String, bool) {
    let mut unwrapped = false;
    let typ = match &field.typ {
        Type::Unit => "None".to_owned(),

        Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64 => "int".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::Float16 | Type::Float32 | Type::Float64 => "float".to_owned(),
        Type::Binary => "bytes".to_owned(),
        Type::String => "str".to_owned(),
        Type::Array {
            elem_type,
            length: _,
        }
        | Type::Vector { elem_type } => match elem_type {
            ElementType::UInt8 => "npt.NDArray[np.uint8]".to_owned(),
            ElementType::UInt16 => "npt.NDArray[np.uint16]".to_owned(),
            ElementType::UInt32 => "npt.NDArray[np.uint32]".to_owned(),
            ElementType::UInt64 => "npt.NDArray[np.uint64]".to_owned(),
            ElementType::Int8 => "npt.NDArray[np.int8]".to_owned(),
            ElementType::Int16 => "npt.NDArray[np.int16]".to_owned(),
            ElementType::Int32 => "npt.NDArray[np.int32]".to_owned(),
            ElementType::Int64 => "npt.NDArray[np.int64]".to_owned(),
            ElementType::Bool => "npt.NDArray[np.bool_]".to_owned(),
            ElementType::Float16 => "npt.NDArray[np.float16]".to_owned(),
            ElementType::Float32 => "npt.NDArray[np.float32]".to_owned(),
            ElementType::Float64 => "npt.NDArray[np.float64]".to_owned(),
            ElementType::Binary => "list[bytes]".to_owned(),
            ElementType::String => "list[str]".to_owned(),
            ElementType::Object { .. } => {
                let typ = quote_type_from_element_type(elem_type);
                if unwrap {
                    unwrapped = true;
                    typ
                } else {
                    format!("list[{typ}]")
                }
            }
        },
        Type::Object { fqname } => quote_type_from_element_type(&ElementType::Object {
            fqname: fqname.clone(),
        }),
    };

    (typ, unwrapped)
}

/// Returns a default converter function for the given field.
///
/// Returns the converter name and, if needed, the converter function itself.
pub fn quote_field_converter_from_field(
    obj: &Object,
    objects: &Objects,
    ext_classes: &ExtensionClasses,
    field: &ObjectField,
) -> (String, String) {
    let mut function = String::new();

    let converter = match &field.typ {
        Type::Unit => {
            panic!("Unit type should only occur for enum variants");
        }
        Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64 => {
            if field.is_nullable {
                "int_or_none".to_owned()
            } else {
                "int".to_owned()
            }
        }
        Type::Bool => {
            if field.is_nullable {
                "bool_or_none".to_owned()
            } else {
                "bool".to_owned()
            }
        }
        Type::Float16 | Type::Float32 | Type::Float64 => {
            if field.is_nullable {
                "float_or_none".to_owned()
            } else {
                "float".to_owned()
            }
        }
        Type::Binary => {
            if field.is_nullable {
                "bytes_or_none".to_owned()
            } else {
                "bytes".to_owned()
            }
        }
        Type::String => {
            if field.is_nullable {
                "str_or_none".to_owned()
            } else {
                "str".to_owned()
            }
        }
        Type::Array {
            elem_type,
            length: _,
        }
        | Type::Vector { elem_type } => match elem_type {
            ElementType::UInt8 => "to_np_uint8".to_owned(),
            ElementType::UInt16 => "to_np_uint16".to_owned(),
            ElementType::UInt32 => "to_np_uint32".to_owned(),
            ElementType::UInt64 => "to_np_uint64".to_owned(),
            ElementType::Int8 => "to_np_int8".to_owned(),
            ElementType::Int16 => "to_np_int16".to_owned(),
            ElementType::Int32 => "to_np_int32".to_owned(),
            ElementType::Int64 => "to_np_int64".to_owned(),
            ElementType::Bool => "to_np_bool".to_owned(),
            ElementType::Float16 => "to_np_float16".to_owned(),
            ElementType::Float32 => "to_np_float32".to_owned(),
            ElementType::Float64 => "to_np_float64".to_owned(),
            _ => String::new(),
        },
        Type::Object { fqname } => {
            let typ = quote_type_from_element_type(&ElementType::Object {
                fqname: fqname.clone(),
            });
            let field_obj = &objects[fqname];

            // If the extension class has a custom init we don't know if we can
            // pass a single argument to it.
            //
            // We generate a default converter only if the field's type can be constructed with a
            // single argument.
            if ext_classes.get(fqname).is_none_or(|c| !c.has_init)
                && (field_obj.fields.len() == 1 || field_obj.is_union())
            {
                let converter_name = format!(
                    "_{}__{}__special_field_converter_override", // TODO(emilk): why does this have an underscore prefix?
                    obj.snake_case_name(),
                    field.name
                );

                // generate the converter function
                if field.is_nullable {
                    function.push_unindented(
                        format!(
                            r#"
                            def {converter_name}(x: {typ}Like | None) -> {typ} | None:
                                if x is None:
                                    return None
                                elif isinstance(x, {typ}):
                                    return x
                                else:
                                    return {typ}(x)
                            "#,
                        ),
                        1,
                    );
                } else {
                    function.push_unindented(
                        format!(
                            r#"
                            def {converter_name}(x: {typ}Like) -> {typ}:
                                if isinstance(x, {typ}):
                                    return x
                                else:
                                    return {typ}(x)
                            "#,
                        ),
                        1,
                    );
                }

                converter_name
            } else {
                String::new()
            }
        }
    };

    (converter, function)
}

pub fn fqname_to_type(fqname: &str) -> String {
    let fqname = fqname.replace(".testing", "");

    let parts = fqname.split('.').collect::<Vec<_>>();

    match parts[..] {
        ["rerun", "datatypes", name] => format!("datatypes.{name}"),
        ["rerun", "components", name] => format!("components.{name}"),
        ["rerun", "archetypes", name] => format!("archetypes.{name}"),
        ["rerun", scope, "datatypes", name] => format!("{scope}_datatypes.{name}"),
        ["rerun", scope, "components", name] => format!("{scope}_components.{name}"),
        ["rerun", scope, "archetypes", name] => format!("{scope}_archetypes.{name}"),
        _ => {
            panic!("Unexpected fqname: {fqname}");
        }
    }
}

pub fn quote_type_from_type(typ: &Type) -> String {
    match typ {
        Type::Unit => {
            panic!("Unit type should only occur for enum variants");
        }

        Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64 => "int".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::Float16 | Type::Float32 | Type::Float64 => "float".to_owned(),
        Type::Binary => "bytes".to_owned(),
        Type::String => "str".to_owned(),
        Type::Object { fqname } => fqname_to_type(fqname),
        Type::Array { elem_type, .. } | Type::Vector { elem_type } => {
            format!(
                "list[{}]",
                quote_type_from_type(&Type::from(elem_type.clone()))
            )
        }
    }
}

pub fn quote_type_from_element_type(typ: &ElementType) -> String {
    quote_type_from_type(&Type::from(typ.clone()))
}

pub fn quote_import_clauses_from_field(
    obj_scope: &Option<String>,
    field: &ObjectField,
) -> Option<String> {
    let fqname = match &field.typ {
        Type::Array {
            elem_type,
            length: _,
        }
        | Type::Vector { elem_type } => match elem_type {
            ElementType::Object { fqname } => Some(fqname),
            _ => None,
        },
        Type::Object { fqname } => Some(fqname),
        _ => None,
    };

    // NOTE: The distinction between `from .` vs. `from rerun.datatypes` has been shown to fix some
    // nasty lazy circular dependencies in weird edge cases…
    // In any case it will be normalized by `ruff` if it turns out to be unnecessary.
    fqname.map(|fqname| quote_import_clauses_from_fqname(obj_scope, fqname))
}

pub fn quote_import_clauses_from_fqname(obj_scope: &Option<String>, fqname: &str) -> String {
    // NOTE: The distinction between `from .` vs. `from rerun.datatypes` has been shown to fix some
    // nasty lazy circular dependencies in weird edge cases…
    // In any case it will be normalized by `ruff` if it turns out to be unnecessary.

    let fqname = fqname.replace(".testing", "");
    let (from, class) = fqname.rsplit_once('.').unwrap_or(("", fqname.as_str()));

    if let Some(scope) = obj_scope {
        if from.starts_with("rerun.datatypes") {
            "from ... import datatypes".to_owned() // NOLINT
        } else if from.starts_with(format!("rerun.{scope}.datatypes").as_str()) {
            format!("from ...{scope} import datatypes as {scope}_datatypes") // NOLINT
        } else if from.starts_with("rerun.components") {
            "from ... import components".to_owned() // NOLINT
        } else if from.starts_with(format!("rerun.{scope}.components").as_str()) {
            format!("from ...{scope} import components as {scope}_components") // NOLINT
        } else if from.starts_with("rerun.archetypes") {
            // NOTE: This is assuming importing other archetypes is legal… which whether it is or
            // isn't for this code generator to say.
            "from ... import archetypes".to_owned() // NOLINT
        } else if from.starts_with(format!("rerun.{scope}.archetypes").as_str()) {
            format!("from ...{scope} import archetypes as {scope}_archetypes") // NOLINT
        } else if from.is_empty() {
            format!("from . import {class}")
        } else {
            format!("from {from} import {class}")
        }
    } else if from.starts_with("rerun.datatypes") {
        "from .. import datatypes".to_owned()
    } else if from.starts_with("rerun.components") {
        "from .. import components".to_owned()
    } else if from.starts_with("rerun.archetypes") {
        // NOTE: This is assuming importing other archetypes is legal… which whether it is or
        // isn't for this code generator to say.
        "from .. import archetypes".to_owned()
    } else if from.is_empty() {
        format!("from . import {class}")
    } else {
        format!("from {from} import {class}")
    }
}

/// Only applies to datatypes and components.
pub fn quote_aliases_from_object(obj: &Object) -> String {
    assert_ne!(obj.kind, ObjectKind::Archetype);

    let aliases = obj.try_get_attr::<String>(ATTR_PYTHON_ALIASES);
    let array_aliases = obj
        .try_get_attr::<String>(ATTR_PYTHON_ARRAY_ALIASES)
        .unwrap_or_default();

    let name = &obj.name;

    let mut code = String::new();

    code.push_unindented(
        &if let Some(aliases) = aliases {
            format!(
                r#"
                if TYPE_CHECKING:
                    {name}Like = {name}{aliases}
                    """A type alias for any {name}-like object."""
                else:
                    {name}Like = Any
                "#,
                aliases = format!(" | {aliases}").trim_end_matches(" | "),
            )
        } else {
            format!(
                r#"
                {name}Like = {name}
                """A type alias for any {name}-like object."""
                "#,
            )
        },
        1,
    );

    code.push_unindented(
        format!(
            r#"
            {name}ArrayLike = {name} | Sequence[{name}Like]{array_aliases}
            """A type alias for any {name}-like array object."""
            "#,
            array_aliases = format!(" | {array_aliases}").trim_end_matches(" | "),
        ),
        0,
    );

    code
}

/// Quote typing aliases for union datatypes. The types for the union arms are automatically
/// included.
pub fn quote_union_aliases_from_object<'a>(
    obj: &Object,
    mut field_types: impl Iterator<Item = &'a String>,
) -> String {
    use itertools::Itertools as _;

    assert_ne!(obj.kind, ObjectKind::Archetype);

    let aliases = obj.try_get_attr::<String>(ATTR_PYTHON_ALIASES);
    let array_aliases = obj
        .try_get_attr::<String>(ATTR_PYTHON_ARRAY_ALIASES)
        .unwrap_or_default();

    let name = &obj.name;

    let union_fields = field_types.join(" | ");
    let aliases = aliases.unwrap_or_default();

    unindent(&format!(
        r#"
            if TYPE_CHECKING:
                {name}Like = {name} | {union_fields}{aliases}
                """A type alias for any {name}-like object."""

                {name}ArrayLike = {name} | {union_fields} | Sequence[{name}Like]{array_aliases}
                """A type alias for any {name}-like array object."""
            else:
                {name}Like = Any
                {name}ArrayLike = Any
            "#,
        aliases = format!(" | {aliases}").trim_end_matches(" | "),
        array_aliases = format!(" | {array_aliases}").trim_end_matches(" | ")
    ))
}

pub fn quote_parameter_type_alias(
    arg_type_fqname: &str,
    class_fqname: &str,
    objects: &Objects,
    array: bool,
) -> String {
    let obj = &objects[arg_type_fqname];

    let base = if let Some(delegate) = obj.delegate_datatype(objects) {
        fqname_to_type(&delegate.fqname)
    } else if arg_type_fqname == class_fqname {
        // We're in the same namespace, so we can use the object name directly.
        // (in fact we have to since we don't import ourselves)
        obj.name.clone()
    } else {
        fqname_to_type(arg_type_fqname)
    };

    if array {
        format!("{base}ArrayLike")
    } else {
        format!("{base}Like")
    }
}
