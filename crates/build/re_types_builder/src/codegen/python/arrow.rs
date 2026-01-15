//! Arrow-related code generation for Python.

use std::collections::{BTreeMap, HashSet};

use itertools::Itertools as _;
use unindent::unindent;

use super::extension_class::{ExtensionClass, NATIVE_TO_PA_ARRAY_METHOD};
use super::object_ext::PythonObjectExt;
use super::typing::quote_field_type_from_field;
use crate::codegen::StringExt as _;
use crate::data_type::{AtomicDataType, DataType, Field, UnionMode};
use crate::{
    Object, ObjectField, ObjectKind, Objects, Reporter, Type, TypeRegistry, ATTR_PYTHON_ALIASES,
};

/// Arrow support objects
///
/// Generated for Components using native types and Datatypes. Components using a Datatype instead
/// delegate to the Datatype's arrow support.
pub fn quote_arrow_support_from_obj(
    reporter: &Reporter,
    type_registry: &TypeRegistry,
    ext_class: &ExtensionClass,
    objects: &Objects,
    obj: &Object,
) -> String {
    let Object { fqname, name, .. } = obj;

    let mut type_superclasses: Vec<String> = vec![];
    let mut batch_superclasses: Vec<String> = vec![];

    let many_aliases = if let Some(data_type) = obj.delegate_datatype(objects) {
        let scope = match data_type.scope() {
            Some(scope) => format!("{scope}."),
            None => String::new(),
        };
        format!("{scope}datatypes{}ArrayLike", data_type.name)
    } else {
        format!("{name}ArrayLike")
    };

    if obj.kind == ObjectKind::Datatype {
        batch_superclasses.push(format!("BaseBatch[{many_aliases}]"));
    } else if obj.kind == ObjectKind::Component {
        if let Some(data_type) = obj.delegate_datatype(objects) {
            let scope = match data_type.scope() {
                Some(scope) => format!("{scope}_"),
                None => String::new(),
            };
            let data_extension_type = format!("{scope}datatypes.{}Type", data_type.name);
            let data_extension_array = format!("{scope}datatypes.{}Batch", data_type.name);
            type_superclasses.push(data_extension_type);
            batch_superclasses.push(data_extension_array);
        } else {
            batch_superclasses.push(format!("BaseBatch[{many_aliases}]"));
        }
        batch_superclasses.push("ComponentBatchMixin".to_owned());
    }

    let datatype = quote_arrow_datatype(&type_registry.get(fqname));
    let extension_batch = format!("{name}Batch");

    let native_to_pa_array_impl = match quote_arrow_serialization(
        reporter,
        objects,
        obj,
        type_registry,
        ext_class,
    ) {
        Ok(automatic_arrow_serialization) => {
            if ext_class.has_native_to_pa_array {
                // There's usually a good reason why serialization is manually implemented,
                // so warning about it is just spam.
                // We could introduce an opt-in flag, but by having a custom method in the first place someone already made the choice.
                if false {
                    reporter.warn(&obj.virtpath, &obj.fqname, format!("No need to manually implement {NATIVE_TO_PA_ARRAY_METHOD} in {} - we can autogenerate the code for this", ext_class.file_name));
                }
                format!(
                    "return {}.{NATIVE_TO_PA_ARRAY_METHOD}(data, data_type)",
                    ext_class.name
                )
            } else {
                automatic_arrow_serialization
            }
        }
        Err(err) => {
            if ext_class.has_native_to_pa_array {
                format!(
                    "return {}.{NATIVE_TO_PA_ARRAY_METHOD}(data, data_type)",
                    ext_class.name
                )
            } else {
                format!(
                    r#"raise NotImplementedError("Arrow serialization of {name} not implemented: {err}") # You need to implement {NATIVE_TO_PA_ARRAY_METHOD} in {}"#,
                    ext_class.file_name
                )
            }
        }
    };

    let batch_superclass_decl = if batch_superclasses.is_empty() {
        String::new()
    } else {
        format!("({})", batch_superclasses.join(","))
    };

    if obj.kind == ObjectKind::Datatype {
        // Datatypes and non-delegating components declare init
        let mut code = unindent(&format!(
            r#"
            class {extension_batch}{batch_superclass_decl}:
                _ARROW_DATATYPE = {datatype}

                @staticmethod
                def _native_to_pa_array(data: {many_aliases}, data_type: pa.DataType) -> pa.Array:
            "#
        ));
        code.push_indented(2, native_to_pa_array_impl, 1);
        code
    } else if obj.is_non_delegating_component() {
        // Datatypes and non-delegating components declare init
        let mut code = unindent(&format!(
            r#"
            class {extension_batch}{batch_superclass_decl}:
                _ARROW_DATATYPE = {datatype}
                _COMPONENT_TYPE: str = "{fqname}"

                @staticmethod
                def _native_to_pa_array(data: {many_aliases}, data_type: pa.DataType) -> pa.Array:
            "#
        ));
        code.push_indented(2, native_to_pa_array_impl, 1);
        code
    } else {
        // Delegating components are already inheriting from their base type
        unindent(&format!(
            r#"
            class {extension_batch}{batch_superclass_decl}:
                _COMPONENT_TYPE: str = "{fqname}"
            "#
        ))
    }
}

pub fn np_dtype_from_type(t: &Type) -> Option<&'static str> {
    match t {
        Type::UInt8 => Some("np.uint8"),
        Type::UInt16 => Some("np.uint16"),
        Type::UInt32 => Some("np.uint32"),
        Type::UInt64 => Some("np.uint64"),
        Type::Int8 => Some("np.int8"),
        Type::Int16 => Some("np.int16"),
        Type::Int32 => Some("np.int32"),
        Type::Int64 => Some("np.int64"),
        Type::Bool => Some("np.bool_"),
        Type::Float16 => Some("np.float16"),
        Type::Float32 => Some("np.float32"),
        Type::Float64 => Some("np.float64"),
        Type::Unit
        | Type::Binary
        | Type::String
        | Type::Array { .. }
        | Type::Vector { .. }
        | Type::Object { .. } => None,
    }
}

/// Only implemented for some cases.
pub fn quote_arrow_serialization(
    reporter: &Reporter,
    objects: &Objects,
    obj: &Object,
    type_registry: &TypeRegistry,
    ext_class: &ExtensionClass,
) -> Result<String, String> {
    use crate::objects::ObjectClass;

    let Object { name, .. } = obj;

    match obj.class {
        ObjectClass::Struct => {
            if obj.is_arrow_transparent() {
                if obj.fields.len() != 1 {
                    reporter.error(
                        &obj.virtpath,
                        &obj.fqname,
                        "Arrow-transparent structs must have exactly one field",
                    );
                } else if obj.fields[0].typ == Type::String {
                    return Ok(unindent(
                        r##"
                            if isinstance(data, str):
                                array: list[str] | npt.ArrayLike = [data]
                            elif isinstance(data, Sequence):
                                array = [str(datum) for datum in data]
                            elif isinstance(data, np.ndarray):
                                array = data
                            else:
                                array = [str(data)]

                            return pa.array(array, type=data_type)
                        "##,
                    ));
                } else if let Some(np_dtype) = np_dtype_from_type(&obj.fields[0].typ) {
                    if !obj.is_attr_set(ATTR_PYTHON_ALIASES) {
                        if !obj.is_testing() {
                            reporter.warn(
                                &obj.virtpath,
                                &obj.fqname,
                                format!("Expected this to have {ATTR_PYTHON_ALIASES} set"),
                            );
                        }
                    } else {
                        return Ok(unindent(&format!(
                            r##"
                                array = np.asarray(data, dtype={np_dtype}).flatten()
                                return pa.array(array, type=data_type)
                            "##
                        )));
                    }
                }
            }

            let mut code = String::new();

            // Would be more correct to also check if the init method has a single parameter here.
            let convert_inner = ext_class.has_init
                && obj
                    .try_get_attr::<String>(ATTR_PYTHON_ALIASES)
                    .is_some_and(|s| !s.is_empty());

            code.push_indented(0, "from typing import cast", 1);
            code.push_indented(
                0,
                quote_local_batch_type_imports(&obj.fields, obj.is_testing()),
                2,
            );

            code.push_indented(0, format!("typed_data: Sequence[{name}]"), 2);

            code.push_indented(0, format!("if isinstance(data, {name}):"), 1);
            code.push_indented(1, "typed_data = [data]", 1);

            code.push_indented(0, "else:", 1);
            if convert_inner {
                code.push_indented(
                    1,
                    format!(
                        "typed_data = [x if isinstance(x, {name}) else {name}(x) for x in data]"
                    ),
                    2,
                );
            } else {
                code.push_indented(1, "typed_data = data", 2);
            }

            code.push_indented(0, "return pa.StructArray.from_arrays(", 1);
            code.push_indented(1, "[", 1);
            for field in &obj.fields {
                let field_name = &field.name;
                let field_array = format!("[x.{field_name} for x in typed_data]");

                match &field.typ {
                    Type::UInt8
                    | Type::UInt16
                    | Type::UInt32
                    | Type::UInt64
                    | Type::Int8
                    | Type::Int16
                    | Type::Int32
                    | Type::Int64
                    | Type::Bool
                    | Type::Float16
                    | Type::Float32
                    | Type::Float64 => {
                        let np_dtype = np_dtype_from_type(&field.typ).unwrap();
                        let field_fwd =
                            format!("pa.array(np.asarray({field_array}, dtype={np_dtype})),");
                        code.push_indented(2, &field_fwd, 1);
                    }

                    Type::Unit
                    | Type::Binary
                    | Type::String
                    | Type::Array { .. }
                    | Type::Vector { .. } => {
                        return Err(
                            "We lack codegen for arrow-serialization of general structs".to_owned()
                        );
                    }
                    Type::Object {
                        fqname: field_fqname,
                    } => {
                        let field_obj = &objects[field_fqname];
                        let field_type_name = &field_obj.name;

                        let field_batch_type = format!("{field_type_name}Batch");

                        // Type checker struggles with this occasionally, exact pattern is unclear.
                        // Tried casting the array earlier via `cast(Sequence[{name}], data)` but to no avail.
                        let field_fwd = format!(
                            "{field_batch_type}({field_array}).as_arrow_array(),  # type: ignore[misc, arg-type]"
                        );
                        code.push_indented(2, &field_fwd, 1);
                    }
                }
            }
            code.push_indented(1, "],", 1);
            code.push_indented(1, "fields=list(data_type),", 1);
            code.push_indented(0, ")", 1);

            Ok(code)
        }

        ObjectClass::Enum(_) => Ok(unindent(&format!(
            r##"
if isinstance(data, ({name}, int, str)):
    data = [data]

pa_data = [{name}.auto(v).value if v is not None else None for v in data] # type: ignore[redundant-expr]

return pa.array(pa_data, type=data_type)
        "##
        ))),

        ObjectClass::Union => {
            let mut variant_list_decls = String::new();
            let mut variant_list_push_arms = String::new();
            let mut child_list_push = String::new();

            // List of all possible types that could be in the incoming data that aren't sequences.
            let mut possible_singular_types = HashSet::new();
            possible_singular_types.insert(name.clone());
            if let Some(aliases) = obj.try_get_attr::<String>(ATTR_PYTHON_ALIASES) {
                possible_singular_types.extend(aliases.split(',').map(|s| s.trim().to_owned()));
            }

            // Checking for the variant and adding it to a flat list.
            // We only have a 'kind' field if the enum is not distinguished by type.
            let type_based_variants = obj
                .fields
                .iter()
                .map(|f| quote_field_type_from_field(objects, f, false).0)
                .all_unique();

            for (idx, field) in obj.fields.iter().enumerate() {
                let kind = field.snake_case_name();
                let variant_kind_list = format!("variant_{kind}");
                let (field_type, _) = quote_field_type_from_field(objects, field, false);

                possible_singular_types.insert(field_type.clone());

                // Build lists of variants.
                let variant_list_decl = if field.typ == Type::Unit {
                    format!("{variant_kind_list}: int = 0")
                } else {
                    format!("{variant_kind_list}: list[{field_type}] = []")
                };
                variant_list_decls.push_unindented(variant_list_decl, 1);

                let if_or_elif = if idx == 0 { "if" } else { "elif" };

                let kind_check = if type_based_variants {
                    format!("isinstance(value.inner, {field_type})")
                } else {
                    format!(r#"value.kind == "{kind}""#)
                };
                variant_list_push_arms.push_indented(2, format!("{if_or_elif} {kind_check}:"), 1);

                let (value_offset_update, append_to_variant_kind_list) = if field.typ == Type::Unit
                {
                    (
                        format!("value_offsets.append({variant_kind_list})"),
                        format!("{variant_kind_list} += 1"),
                    )
                } else {
                    let ignore_type_check = if type_based_variants {
                        ""
                    } else {
                        // my-py doesn't know that this has the right type now.
                        "# type: ignore[arg-type]"
                    };
                    (
                        format!("value_offsets.append(len({variant_kind_list}))"),
                        format!("{variant_kind_list}.append(value.inner) {ignore_type_check}"),
                    )
                };
                variant_list_push_arms.push_indented(3, value_offset_update, 1);
                variant_list_push_arms.push_indented(3, append_to_variant_kind_list, 1);
                variant_list_push_arms.push_indented(3, format!("types.append({})", idx + 1), 1); // 0 is reserved for nulls

                // Converting the variant list to a pa array.
                let variant_list_to_pa_array = match &field.typ {
                    Type::Object { fqname } => {
                        let field_type_name = &objects[fqname].name;
                        format!("{field_type_name}Batch({variant_kind_list}).as_arrow_array()")
                    }
                    Type::Unit => {
                        format!("pa.nulls({variant_kind_list})")
                    }
                    Type::UInt8
                    | Type::UInt16
                    | Type::UInt32
                    | Type::UInt64
                    | Type::Int8
                    | Type::Int16
                    | Type::Int32
                    | Type::Int64
                    | Type::Bool
                    | Type::Float16
                    | Type::Float32
                    | Type::Float64
                    | Type::Binary
                    | Type::String => {
                        let datatype = quote_arrow_datatype(&type_registry.get(&field.fqname));
                        format!("pa.array({variant_kind_list}, type={datatype})")
                    }
                    Type::Array { .. } | Type::Vector { .. } => {
                        return Err(format!(
                            "We lack codegen for arrow-serialization of unions containing lists. Can't handle type {}",
                            field.fqname
                        ));
                    }
                };
                child_list_push.push_indented(1, format!("{variant_list_to_pa_array},"), 1);
            }

            let singular_checks = possible_singular_types
                .into_iter()
                .sorted() // Make order not dependent on hash shenanigans (also looks nicer often).
                .filter(|typename| !typename.contains('[')) // If we keep these we unfortunately get: `TypeError: Subscripted generics cannot be used with class and instance checks`
                .filter(|typename| !typename.ends_with("Like")) // TODO(#10959): `xLike` types are union types and checking those is not supported until Python 3.10.
                .map(|typename| {
                    if typename == "None" {
                        "type(None)".to_owned() // TODO(#10959): `NoneType` requires Python 3.10.
                    } else {
                        typename
                    }
                })
                .join(", ");

            let batch_type_imports = quote_local_batch_type_imports(&obj.fields, obj.is_testing());
            Ok(format!(
                r##"
{batch_type_imports}
from typing import cast

if not hasattr(data, "__iter__") or isinstance(data, ({singular_checks})): # type: ignore[arg-type]
    data = [data] # type: ignore[list-item]
data = cast(Sequence[{name}Like], data) # type: ignore[redundant-cast]

types: list[int] = []
value_offsets: list[int] = []

num_nulls = 0
{variant_list_decls}

for value in data:
    if value is None:
        value_offsets.append(num_nulls)
        num_nulls += 1
        types.append(0)
    else:
        if not isinstance(value, {name}):
            value = {name}(value)
{variant_list_push_arms}

buffers = [
    None,
    pa.array(types, type=pa.int8()).buffers()[1],
    pa.array(value_offsets, type=pa.int32()).buffers()[1],
]
children = [
    pa.nulls(num_nulls),
{child_list_push}
]

return pa.UnionArray.from_buffers(
    type=data_type,
    length=len(data),
    buffers=buffers,
    children=children,
)
        "##
            ))
        }
    }
}

pub fn quote_local_batch_type_imports(fields: &[ObjectField], current_obj_is_testing: bool) -> String {
    let mut code = String::new();

    for field in fields {
        let Type::Object {
            fqname: field_fqname,
        } = &field.typ
        else {
            continue;
        };
        if let Some(last_dot) = field_fqname.rfind('.') {
            let mod_path = &field_fqname[..last_dot];
            let field_type_name = &field_fqname[last_dot + 1..];

            // If both the current object and the field object are testing types,
            // use relative imports instead of absolute ones
            let is_field_testing = crate::objects::is_testing_fqname(field_fqname);
            let import_path = if current_obj_is_testing && is_field_testing {
                // Extract the relative path within the testing namespace
                if let Some(testing_prefix) = mod_path.strip_prefix("rerun.testing.datatypes") {
                    format!(".{testing_prefix}")
                } else if mod_path == "rerun.testing" {
                    ".".to_owned()
                } else {
                    mod_path.to_owned()
                }
            } else {
                mod_path.to_owned()
            };

            code.push_unindented(
                format!("from {import_path} import {field_type_name}Batch"),
                1,
            );
        }
    }
    code
}

pub fn quote_arrow_datatype(datatype: &DataType) -> String {
    match datatype {
        DataType::Atomic(AtomicDataType::Null) => "pa.null()".to_owned(),
        DataType::Atomic(AtomicDataType::Boolean) => "pa.bool_()".to_owned(),
        DataType::Atomic(AtomicDataType::Int8) => "pa.int8()".to_owned(),
        DataType::Atomic(AtomicDataType::Int16) => "pa.int16()".to_owned(),
        DataType::Atomic(AtomicDataType::Int32) => "pa.int32()".to_owned(),
        DataType::Atomic(AtomicDataType::Int64) => "pa.int64()".to_owned(),
        DataType::Atomic(AtomicDataType::UInt8) => "pa.uint8()".to_owned(),
        DataType::Atomic(AtomicDataType::UInt16) => "pa.uint16()".to_owned(),
        DataType::Atomic(AtomicDataType::UInt32) => "pa.uint32()".to_owned(),
        DataType::Atomic(AtomicDataType::UInt64) => "pa.uint64()".to_owned(),
        DataType::Atomic(AtomicDataType::Float16) => "pa.float16()".to_owned(),
        DataType::Atomic(AtomicDataType::Float32) => "pa.float32()".to_owned(),
        DataType::Atomic(AtomicDataType::Float64) => "pa.float64()".to_owned(),

        DataType::Binary => "pa.large_binary()".to_owned(),

        DataType::Utf8 => "pa.utf8()".to_owned(),

        DataType::List(field) => {
            let field = quote_arrow_field(field);
            format!("pa.list_({field})")
        }

        DataType::FixedSizeList(field, length) => {
            let field = quote_arrow_field(field);
            format!("pa.list_({field}, {length})")
        }

        DataType::Union(fields, mode) => {
            let fields = fields
                .iter()
                .map(quote_arrow_field)
                .collect::<Vec<_>>()
                .join(", ");
            match mode {
                UnionMode::Dense => format!(r#"pa.dense_union([{fields}])"#),
                UnionMode::Sparse => format!(r#"pa.sparse_union([{fields}])"#),
            }
        }

        DataType::Struct(fields) => {
            let fields = fields
                .iter()
                .map(quote_arrow_field)
                .collect::<Vec<_>>()
                .join(", ");
            format!("pa.struct([{fields}])")
        }

        DataType::Object { datatype, .. } => quote_arrow_datatype(datatype),
    }
}

pub fn quote_arrow_field(field: &Field) -> String {
    let Field {
        name,
        data_type,
        is_nullable,
        metadata,
    } = field;

    let datatype = quote_arrow_datatype(data_type);
    let is_nullable = *is_nullable || matches!(data_type.to_logical_type(), DataType::Union { .. }); // Rerun unions always has a `_null_marker: null` variant, so they are always nullable
    let is_nullable = if is_nullable { "True" } else { "False" };
    let metadata = quote_metadata_map(metadata);

    format!(r#"pa.field("{name}", {datatype}, nullable={is_nullable}, metadata={metadata})"#)
}

pub fn quote_metadata_map(metadata: &BTreeMap<String, String>) -> String {
    let kvs = metadata
        .iter()
        .map(|(k, v)| format!("{k:?}, {v:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{kvs}}}")
}
