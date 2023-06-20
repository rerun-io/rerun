//! Implements the Python codegen pass.

use anyhow::Context as _;
use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{StringExt as _, AUTOGEN_WARNING},
    ArrowRegistry, CodeGenerator, Docs, ElementType, Object, ObjectField, ObjectKind, Objects,
    Type, ATTR_PYTHON_ALIASES, ATTR_PYTHON_ARRAY_ALIASES, ATTR_PYTHON_TRANSPARENT,
    ATTR_RERUN_LEGACY_FQNAME,
};

// ---

pub struct PythonCodeGenerator {
    pkg_path: PathBuf,
}

impl PythonCodeGenerator {
    pub fn new(pkg_path: impl Into<PathBuf>) -> Self {
        Self {
            pkg_path: pkg_path.into(),
        }
    }
}

impl CodeGenerator for PythonCodeGenerator {
    fn generate(&mut self, objs: &Objects, arrow_registry: &ArrowRegistry) -> Vec<PathBuf> {
        let mut filepaths = Vec::new();

        let datatypes_path = self.pkg_path.join("datatypes");
        std::fs::create_dir_all(&datatypes_path)
            .with_context(|| format!("{datatypes_path:?}"))
            .unwrap();
        filepaths.extend(
            quote_objects(
                datatypes_path,
                arrow_registry,
                objs,
                &objs.ordered_objects(ObjectKind::Datatype.into()),
            )
            .0,
        );

        let components_path = self.pkg_path.join("components");
        std::fs::create_dir_all(&components_path)
            .with_context(|| format!("{components_path:?}"))
            .unwrap();
        filepaths.extend(
            quote_objects(
                components_path,
                arrow_registry,
                objs,
                &objs.ordered_objects(ObjectKind::Component.into()),
            )
            .0,
        );

        let archetypes_path = self.pkg_path.join("archetypes");
        std::fs::create_dir_all(&archetypes_path)
            .with_context(|| format!("{archetypes_path:?}"))
            .unwrap();
        let (paths, archetype_names) = quote_objects(
            archetypes_path,
            arrow_registry,
            objs,
            &objs.ordered_objects(ObjectKind::Archetype.into()),
        );
        filepaths.extend(paths);

        filepaths.push(quote_lib(&self.pkg_path, &archetype_names));

        filepaths
    }
}

// --- File management ---

fn quote_lib(out_path: impl AsRef<Path>, archetype_names: &[String]) -> PathBuf {
    let out_path = out_path.as_ref();

    std::fs::create_dir_all(out_path)
        .with_context(|| format!("{out_path:?}"))
        .unwrap();

    let path = out_path.join("__init__.py");
    let manifest = quote_manifest(archetype_names);
    let archetype_names = archetype_names.join(", ");

    let mut code = String::new();

    code += &unindent::unindent(&format!(
        r#"
        # {AUTOGEN_WARNING}

        from __future__ import annotations

        __all__ = [{manifest}]

        from .archetypes import {archetype_names}
        "#
    ));

    std::fs::write(&path, code)
        .with_context(|| format!("{path:?}"))
        .unwrap();

    path
}

/// Returns all filepaths + all object names.
fn quote_objects(
    out_path: impl AsRef<Path>,
    arrow_registry: &ArrowRegistry,
    all_objects: &Objects,
    objs: &[&Object],
) -> (Vec<PathBuf>, Vec<String>) {
    let out_path = out_path.as_ref();

    let mut filepaths = Vec::new();
    let mut all_names = Vec::new();

    let mut files = HashMap::<PathBuf, Vec<QuotedObject>>::new();
    for obj in objs {
        all_names.push(obj.name.clone());

        let obj = if obj.is_struct() {
            QuotedObject::from_struct(arrow_registry, all_objects, obj)
        } else {
            QuotedObject::from_union(arrow_registry, all_objects, obj)
        };

        let filepath = out_path.join(obj.filepath.file_name().unwrap());
        files.entry(filepath.clone()).or_default().push(obj);
    }

    // (module_name, [object_name])
    let mut mods = HashMap::<String, Vec<String>>::new();

    // rerun/{datatypes|components|archetypes}/{xxx}.py
    for (filepath, objs) in files {
        let names = objs
            .iter()
            .flat_map(|obj| match obj.kind {
                ObjectKind::Datatype | ObjectKind::Component => {
                    let name = &obj.name;

                    vec![
                        format!("{name}"),
                        format!("{name}Like"),
                        format!("{name}Array"),
                        format!("{name}ArrayLike"),
                        format!("{name}Type"),
                    ]
                }
                ObjectKind::Archetype => vec![obj.name.clone()],
            })
            .collect::<Vec<_>>();

        // NOTE: Isolating the file stem only works because we're handling datatypes, components
        // and archetypes separately (and even then it's a bit shady, eh).
        mods.entry(filepath.file_stem().unwrap().to_string_lossy().to_string())
            .or_default()
            .extend(names.iter().cloned());

        filepaths.push(filepath.clone());
        let mut file = std::fs::File::create(&filepath)
            .with_context(|| format!("{filepath:?}"))
            .unwrap();

        let mut code = String::new();
        code.push_text(&format!("# {AUTOGEN_WARNING}"), 2, 0);

        let manifest = quote_manifest(names);
        code.push_unindented_text(
            format!(
                "
                from __future__ import annotations

                __all__ = [{manifest}]

                ",
            ),
            0,
        );

        for obj in objs {
            code.push_text(&obj.code, 1, 0);
        }
        file.write_all(code.as_bytes())
            .with_context(|| format!("{filepath:?}"))
            .unwrap();
    }

    // rerun/{datatypes|components|archetypes}/__init__.py
    {
        let path = out_path.join("__init__.py");

        let mut code = String::new();

        let manifest = quote_manifest(mods.iter().flat_map(|(_, names)| names.iter()));

        code.push_text(&format!("# {AUTOGEN_WARNING}"), 2, 0);
        code.push_unindented_text(
            format!(
                "
                from __future__ import annotations

                __all__ = [{manifest}]

                ",
            ),
            0,
        );

        for (module, names) in &mods {
            let names = names.join(", ");
            code.push_text(&format!("from .{module} import {names}"), 1, 0);
        }

        filepaths.push(path.clone());
        std::fs::write(&path, code)
            .with_context(|| format!("{path:?}"))
            .unwrap();
    }

    (filepaths, all_names)
}

// --- Codegen core loop ---

#[derive(Debug, Clone)]
struct QuotedObject {
    filepath: PathBuf,
    name: String,
    kind: ObjectKind,
    code: String,
}

impl QuotedObject {
    fn from_struct(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> Self {
        assert!(obj.is_struct());

        let Object {
            filepath,
            fqname: _,
            pkg_name: _,
            name,
            docs,
            kind,
            attrs: _,
            fields,
            specifics: _,
        } = obj;

        let mut code = String::new();

        code.push_text(&quote_module_prelude(), 0, 0);

        for clause in obj
            .fields
            .iter()
            .filter_map(quote_import_clauses_from_field)
        {
            code.push_text(&clause, 1, 0);
        }

        code.push_unindented_text(
            format!(
                r#"

                @dataclass
                class {name}:
                "#
            ),
            0,
        );

        code.push_text(quote_doc_from_docs(docs), 0, 4);

        for field in fields {
            let ObjectField {
                filepath: _,
                fqname: _,
                pkg_name: _,
                name,
                docs,
                typ: _,
                attrs: _,
                required: _,
                deprecated: _,
            } = field;

            let (typ, _) = quote_field_type_from_field(objects, field, false);
            let typ = if *kind == ObjectKind::Archetype {
                let (typ_unwrapped, _) = quote_field_type_from_field(objects, field, true);
                format!("{typ_unwrapped}Array")
            } else {
                typ
            };
            let typ = if field.required {
                typ
            } else {
                format!("{typ} | None = None")
            };

            code.push_text(format!("{name}: {typ}"), 1, 4);

            code.push_text(quote_doc_from_docs(docs), 0, 4);
        }

        code.push_text(quote_str_repr_from_obj(obj), 1, 4);
        code.push_text(quote_array_method_from_obj(objects, obj), 1, 4);
        code.push_text(quote_str_method_from_obj(objects, obj), 1, 4);

        if obj.kind == ObjectKind::Archetype {
            code.push_text(quote_builder_from_obj(objects, obj), 1, 4);
        } else {
            code.push_text(quote_aliases_from_object(obj), 1, 0);
        }

        code.push_text(quote_arrow_support_from_obj(arrow_registry, obj), 1, 0);

        let mut filepath = PathBuf::from(filepath);
        filepath.set_extension("py");

        Self {
            filepath,
            name: obj.name.clone(),
            kind: obj.kind,
            code,
        }
    }

    fn from_union(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> Self {
        assert!(!obj.is_struct());

        let Object {
            filepath,
            fqname: _,
            pkg_name: _,
            name,
            docs,
            kind: _,
            attrs: _,
            fields,
            specifics: _,
        } = obj;

        let mut code = String::new();

        code.push_text(&quote_module_prelude(), 0, 0);

        for clause in obj
            .fields
            .iter()
            .filter_map(quote_import_clauses_from_field)
        {
            code.push_text(&clause, 1, 0);
        }

        code.push_unindented_text(
            format!(
                r#"

                @dataclass
                class {name}:
                "#
            ),
            0,
        );

        code.push_text(quote_doc_from_docs(docs), 0, 4);

        for field in fields {
            let ObjectField {
                filepath: _,
                fqname: _,
                pkg_name: _,
                name,
                docs,
                typ: _,
                attrs: _,
                required: _,
                deprecated: _,
            } = field;

            let (typ, _) = quote_field_type_from_field(objects, field, false);
            // NOTE: It's always optional since only one of the fields can be set at a time.
            code.push_text(format!("{name}: {typ} | None = None"), 1, 4);

            code.push_text(quote_doc_from_docs(docs), 0, 4);
        }

        code.push_text(quote_str_repr_from_obj(obj), 1, 4);
        code.push_text(quote_array_method_from_obj(objects, obj), 1, 4);
        code.push_text(quote_str_method_from_obj(objects, obj), 1, 4);

        code.push_text(quote_aliases_from_object(obj), 1, 0);
        code.push_text(quote_arrow_support_from_obj(arrow_registry, obj), 1, 0);

        let mut filepath = PathBuf::from(filepath);
        filepath.set_extension("py");

        Self {
            filepath,
            name: obj.name.clone(),
            kind: obj.kind,
            code,
        }
    }
}

// --- Code generators ---

fn quote_manifest(names: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut quoted_names: Vec<_> = names
        .into_iter()
        .map(|name| format!("{:?}", name.as_ref()))
        .collect();
    quoted_names.sort();

    quoted_names.join(", ")
}

fn quote_module_prelude() -> String {
    // NOTE: All the extraneous stuff will be cleaned up courtesy of `ruff`.
    unindent::unindent(
        r#"
        import numpy as np
        import numpy.typing as npt
        import pyarrow as pa

        from dataclasses import dataclass
        from typing import Any, Dict, Iterable, List, Optional, Sequence, Set, Tuple, Union

        "#,
    )
}

fn quote_doc_from_docs(docs: &Docs) -> String {
    let lines = crate::codegen::get_documentation(docs, &["py", "python"]);

    if lines.is_empty() {
        return String::new();
    }

    let doc = lines.join("\n");
    format!("\"\"\"\n{doc}\n\"\"\"\n\n")
}

/// Generates generic `__str__` and `__repr__` methods for archetypes.
//
// TODO(cmc): this could alternatively import a statically defined mixin from "somewhere".
fn quote_str_repr_from_obj(obj: &Object) -> String {
    if obj.kind != ObjectKind::Archetype {
        return String::new();
    }

    unindent::unindent(
        r#"
        def __str__(self) -> str:
            s = f"rr.{type(self).__name__}(\n"

            from dataclasses import fields
            for field in fields(self):
                data = getattr(self, field.name)
                datatype = getattr(data, "type", None)
                if datatype:
                    name = datatype.extension_name
                    typ = datatype.storage_type
                    s += f"  {name}<{typ}>(\n    {data.to_pylist()}\n  )\n"

            s += ")"

            return s

        def __repr__(self) -> str:
            return str(self)

        "#,
    )
}

/// Automatically implement `__array__` if the object is a single
/// `npt.ArrayLike`/integer/floating-point field.
///
/// Only applies to datatypes and components.
fn quote_array_method_from_obj(objects: &Objects, obj: &Object) -> String {
    // TODO(cmc): should be using native type, but need transparency
    let typ = quote_field_type_from_field(objects, &obj.fields[0], false).0;
    if
    // cannot be an archetype
    obj.kind == ObjectKind::Archetype
        // has to have a single field
        || obj.fields.len() != 1
        // that single field must be `npt.ArrayLike`/integer/floating-point
        || !["npt.ArrayLike", "float", "int"].contains(&typ.as_str())
    {
        return String::new();
    }

    let field_name = &obj.fields[0].name;
    unindent::unindent(&format!(
        "
        def __array__(self) -> npt.ArrayLike:
            return np.asarray(self.{field_name})
        ",
    ))
}

/// Automatically implement `__str__` if the object is a single `str` field.
///
/// Only applies to datatypes and components.
fn quote_str_method_from_obj(objects: &Objects, obj: &Object) -> String {
    if
    // cannot be an archetype
    obj.kind == ObjectKind::Archetype
        // has to have a single field
        || obj.fields.len() != 1
        // that single field must be `str`
        // TODO(cmc): should be using native type, but need transparency
        || quote_field_type_from_field(objects, &obj.fields[0], false).0 != "str"
    {
        return String::new();
    }

    let field_name = &obj.fields[0].name;
    unindent::unindent(&format!(
        "
        def __str__(self) -> str:
            return self.{field_name}
        ",
    ))
}

/// Only applies to datatypes and components.
fn quote_aliases_from_object(obj: &Object) -> String {
    assert!(obj.kind != ObjectKind::Archetype);

    let aliases = obj.try_get_attr::<String>(ATTR_PYTHON_ALIASES);
    let array_aliases = obj
        .try_get_attr::<String>(ATTR_PYTHON_ARRAY_ALIASES)
        .unwrap_or_default();

    let name = &obj.name;

    let mut code = String::new();

    code.push_unindented_text(
        &if let Some(aliases) = aliases {
            format!(
                r#"
                {name}Like = Union[
                    {name},
                    {aliases}
                ]
                "#,
            )
        } else {
            format!("{name}Like = {name}")
        },
        1,
    );

    code.push_unindented_text(
        format!(
            r#"
            {name}ArrayLike = Union[
                {name}Like,
                Sequence[{name}Like],
                {array_aliases}
            ]
            "#,
        ),
        0,
    );

    code
}

fn quote_import_clauses_from_field(field: &ObjectField) -> Option<String> {
    let fqname = match &field.typ {
        Type::Array {
            elem_type,
            length: _,
        }
        | Type::Vector { elem_type } => match elem_type {
            ElementType::Object(fqname) => Some(fqname),
            _ => None,
        },
        Type::Object(fqname) => Some(fqname),
        _ => None,
    };

    // NOTE: The distinction between `from .` vs. `from rerun.datatypes` has been shown to fix some
    // nasty lazy circular dependencies in weird edge cases...
    // In any case it will be normalized by `ruff` if it turns out to be unnecessary.
    fqname.map(|fqname| {
        let (from, class) = fqname.rsplit_once('.').unwrap_or(("", fqname.as_str()));
        if from.starts_with("rerun.datatypes") {
            "from .. import datatypes".to_owned()
        } else if from.starts_with("rerun.components") {
            "from .. import components".to_owned()
        } else if from.starts_with("rerun.archetypes") {
            // NOTE: This is assuming importing other archetypes is legal... which whether it is or
            // isn't for this code generator to say.
            "from .. import archetypes".to_owned()
        } else if from.is_empty() {
            format!("from . import {class}")
        } else {
            format!("from {from} import {class}")
        }
    })
}

/// Returns type name as string and whether it was force unwrapped.
///
/// Specifying `unwrap = true` will unwrap the final type before returning it, e.g. `Vec<String>`
/// becomes just `String`.
/// The returned boolean indicates whether there was anything to unwrap at all.
fn quote_field_type_from_field(
    objects: &Objects,
    field: &ObjectField,
    unwrap: bool,
) -> (String, bool) {
    let mut unwrapped = false;
    let typ = match &field.typ {
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
        Type::String => "str".to_owned(),
        Type::Array {
            elem_type,
            length: _,
        }
        | Type::Vector { elem_type } => {
            let array_like = matches!(
                elem_type,
                ElementType::UInt8
                    | ElementType::UInt16
                    | ElementType::UInt32
                    | ElementType::UInt64
                    | ElementType::Int8
                    | ElementType::Int16
                    | ElementType::Int32
                    | ElementType::Int64
                    | ElementType::Bool
                    | ElementType::Float16
                    | ElementType::Float32
                    | ElementType::Float64
                    | ElementType::String
            );

            if array_like {
                "npt.ArrayLike".to_owned()
            } else {
                let typ = quote_type_from_element_type(elem_type);
                if unwrap {
                    unwrapped = true;
                    typ
                } else {
                    format!("List[{typ}]")
                }
            }
        }
        Type::Object(fqname) => {
            // TODO(cmc): it is a bit weird to be doing the transparency logic (which is language
            // agnostic) in a python specific quoting function... a static helper at the very least
            // would be nice.
            let is_transparent = field
                .try_get_attr::<String>(ATTR_PYTHON_TRANSPARENT)
                .is_some();
            if is_transparent {
                let target = objects.get(fqname);
                assert!(
                    target.fields.len() == 1,
                    "transparent field must point to an object with exactly 1 field, but {:?} has {}",
                    fqname, target.fields.len(),
                );
                // NOTE: unwrap call is safe due to assertion just above
                return quote_field_type_from_field(
                    objects,
                    target.fields.first().unwrap(),
                    unwrap,
                );
            }
            quote_type_from_element_type(&ElementType::Object(fqname.clone()))
        }
    };

    (typ, unwrapped)
}

fn quote_type_from_element_type(typ: &ElementType) -> String {
    match typ {
        ElementType::UInt8
        | ElementType::UInt16
        | ElementType::UInt32
        | ElementType::UInt64
        | ElementType::Int8
        | ElementType::Int16
        | ElementType::Int32
        | ElementType::Int64 => "int".to_owned(),
        ElementType::Bool => "bool".to_owned(),
        ElementType::Float16 | ElementType::Float32 | ElementType::Float64 => "float".to_owned(),
        ElementType::String => "str".to_owned(),
        ElementType::Object(fqname) => {
            let (from, class) = fqname.rsplit_once('.').unwrap_or(("", fqname.as_str()));
            if from.starts_with("rerun.datatypes") {
                format!("datatypes.{class}")
            } else if from.starts_with("rerun.components") {
                format!("components.{class}")
            } else if from.starts_with("rerun.archetypes") {
                // NOTE: This is assuming importing other archetypes is legal... which whether it is or
                // isn't for this code generator to say.
                format!("archetypes.{class}")
            } else if from.is_empty() {
                format!("from . import {class}")
            } else {
                format!("from {from} import {class}")
            }
        }
    }
}

fn quote_arrow_support_from_obj(arrow_registry: &ArrowRegistry, obj: &Object) -> String {
    let Object {
        fqname, name, kind, ..
    } = obj;

    match kind {
        ObjectKind::Datatype | ObjectKind::Component => {
            let datatype = quote_arrow_datatype(&arrow_registry.get(fqname));

            let mono = name.clone();
            let mono_aliases = format!("{name}Like");
            let many = format!("{name}Array");
            let many_aliases = format!("{name}ArrayLike");
            let arrow = format!("{name}Type");

            use convert_case::{Boundary, Case, Casing};
            let pkg = name
                .from_case(Case::Camel)
                .without_boundaries(&[
                    Boundary::DigitLower,
                    Boundary::DigitUpper,
                    Boundary::LowerDigit,
                    Boundary::UpperDigit,
                ])
                .to_case(Case::Snake);

            let legacy_fqname = obj
                .try_get_attr::<String>(ATTR_RERUN_LEGACY_FQNAME)
                .unwrap_or_else(|| fqname.clone());

            unindent::unindent(&format!(
                r#"

                # --- Arrow support ---

                from .{pkg}_ext import {many}Ext # noqa: E402

                class {arrow}(pa.ExtensionType): # type: ignore[misc]
                    def __init__(self: type[pa.ExtensionType]) -> None:
                        pa.ExtensionType.__init__(
                            self, {datatype}, "{legacy_fqname}"
                        )

                    def __arrow_ext_serialize__(self: type[pa.ExtensionType]) -> bytes:
                        # since we don't have a parameterized type, we don't need extra metadata to be deserialized
                        return b""

                    @classmethod
                    def __arrow_ext_deserialize__(
                        cls: type[pa.ExtensionType], storage_type: Any, serialized: Any
                    ) -> type[pa.ExtensionType]:
                        # return an instance of this subclass given the serialized metadata.
                        return {arrow}()

                    def __arrow_ext_class__(self: type[pa.ExtensionType]) -> type[pa.ExtensionArray]:
                        return {many}

                # TODO(cmc): bring back registration to pyarrow once legacy types are gone
                # pa.register_extension_type({arrow}())

                class {many}(pa.ExtensionArray, {many}Ext):  # type: ignore[misc]
                    @staticmethod
                    def from_similar(data: {many_aliases} | None) -> pa.Array:
                        if data is None:
                            return {arrow}().wrap_array(pa.array([], type={arrow}().storage_type))
                        else:
                            return {many}Ext._from_similar(
                                data,
                                mono={mono},
                                mono_aliases={mono_aliases},
                                many={many},
                                many_aliases={many_aliases},
                                arrow={arrow},
                            )
                "#
            ))
        }
        ObjectKind::Archetype => String::new(),
    }
}

/// Only makes sense for archetypes.
fn quote_builder_from_obj(objects: &Objects, obj: &Object) -> String {
    assert_eq!(ObjectKind::Archetype, obj.kind);

    let required = obj
        .fields
        .iter()
        .filter(|field| field.required)
        .collect::<Vec<_>>();
    let optional = obj
        .fields
        .iter()
        .filter(|field| !field.required)
        .collect::<Vec<_>>();

    let mut code = String::new();

    let required_args = required
        .iter()
        .map(|field| {
            let (typ, unwrapped) = quote_field_type_from_field(objects, field, true);
            if unwrapped {
                // This was originally a vec/array!
                format!("{}: {typ}ArrayLike", field.name)
            } else {
                format!("{}: {typ}Like", field.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let optional_args = optional
        .iter()
        .map(|field| {
            let (typ, unwrapped) = quote_field_type_from_field(objects, field, true);
            if unwrapped {
                // This was originally a vec/array!
                format!("{}: {typ}ArrayLike | None = None", field.name)
            } else {
                format!("{}: {typ}Like | None = None", field.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    code.push_text(
        format!("def __init__(self, {required_args}, *, {optional_args}) -> None:"),
        1,
        0,
    );

    code.push_text("# Required components", 1, 4);
    for field in required {
        let name = &field.name;
        let (typ, _) = quote_field_type_from_field(objects, field, true);
        code.push_text(
            format!("self.{name} = {typ}Array.from_similar({name})"),
            1,
            4,
        );
    }

    code.push('\n');

    code.push_text("# Optional components\n", 1, 4);
    for field in optional {
        let name = &field.name;
        let (typ, _) = quote_field_type_from_field(objects, field, true);
        code.push_text(
            format!("self.{name} = {typ}Array.from_similar({name})"),
            1,
            4,
        );
    }

    code
}

// --- Arrow registry code generators ---

use arrow2::datatypes::{DataType, Field, UnionMode};

fn quote_arrow_datatype(datatype: &DataType) -> String {
    match datatype {
        DataType::Null => "pa.null()".to_owned(),
        DataType::Boolean => "pa.bool_()".to_owned(),
        DataType::Int8 => "pa.int8()".to_owned(),
        DataType::Int16 => "pa.int16()".to_owned(),
        DataType::Int32 => "pa.int32()".to_owned(),
        DataType::Int64 => "pa.int64()".to_owned(),
        DataType::UInt8 => "pa.uint8()".to_owned(),
        DataType::UInt16 => "pa.uint16()".to_owned(),
        DataType::UInt32 => "pa.uint32()".to_owned(),
        DataType::UInt64 => "pa.uint64()".to_owned(),
        DataType::Float16 => "pa.float16()".to_owned(),
        DataType::Float32 => "pa.float32()".to_owned(),
        DataType::Float64 => "pa.float64()".to_owned(),
        DataType::Date32 => "pa.date32()".to_owned(),
        DataType::Date64 => "pa.date64()".to_owned(),
        DataType::Binary => "pa.binary()".to_owned(),
        DataType::LargeBinary => "pa.large_binary()".to_owned(),
        DataType::Utf8 => "pa.utf8()".to_owned(),
        DataType::LargeUtf8 => "pa.large_utf8()".to_owned(),
        DataType::FixedSizeList(field, length) => {
            let field = quote_arrow_field(field);
            format!("pa.list_({field}, {length})")
        }
        DataType::Union(fields, _, mode) => {
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
        DataType::Extension(_, datatype, _) => quote_arrow_datatype(datatype),
        _ => unimplemented!("{datatype:#?}"), // NOLINT
    }
}

fn quote_arrow_field(field: &Field) -> String {
    let Field {
        name,
        data_type,
        is_nullable,
        metadata,
    } = field;

    let datatype = quote_arrow_datatype(data_type);
    let is_nullable = is_nullable.then_some("True").unwrap_or("False");
    let metadata = quote_metadata_map(metadata);

    format!(r#"pa.field("{name}", {datatype}, {is_nullable}, {metadata})"#)
}

fn quote_metadata_map(metadata: &BTreeMap<String, String>) -> String {
    let kvs = metadata
        .iter()
        .map(|(k, v)| format!("{k:?}, {v:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{kvs}}}")
}
