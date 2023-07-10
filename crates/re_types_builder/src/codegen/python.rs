//! Implements the Python codegen pass.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Write,
};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;

use crate::{
    codegen::{StringExt as _, AUTOGEN_WARNING},
    ArrowRegistry, CodeGenerator, Docs, ElementType, Object, ObjectField, ObjectKind, Objects,
    Type, ATTR_PYTHON_ALIASES, ATTR_PYTHON_ARRAY_ALIASES, ATTR_RERUN_LEGACY_FQNAME,
};

// ---

/// Python-specific helpers for [`Object`].
trait PythonObjectExt {
    /// Returns `true` if the object is a delegating component.
    ///
    /// Components can either use a native type, or a custom datatype. In the latter case, the
    /// the component delegates its implementation to the datatype.
    fn is_delegating_component(&self) -> bool;

    /// Returns `true` if the object is a non-delegating component.
    fn is_non_delegating_component(&self) -> bool;

    /// If the object is a delegating component, returns the datatype it delegates to.
    fn delegate_datatype<'a>(&self, objects: &'a Objects) -> Option<&'a Object>;
}

impl PythonObjectExt for Object {
    fn is_delegating_component(&self) -> bool {
        self.kind == ObjectKind::Component && matches!(self.fields[0].typ, Type::Object(_))
    }

    fn is_non_delegating_component(&self) -> bool {
        self.kind == ObjectKind::Component && !self.is_delegating_component()
    }

    fn delegate_datatype<'a>(&self, objects: &'a Objects) -> Option<&'a Object> {
        self.is_delegating_component()
            .then(|| {
                if let Type::Object(name) = &self.fields[0].typ {
                    Some(objects.get(name))
                } else {
                    None
                }
            })
            .flatten()
    }
}

pub struct PythonCodeGenerator {
    pkg_path: Utf8PathBuf,
}

impl PythonCodeGenerator {
    pub fn new(pkg_path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            pkg_path: pkg_path.into(),
        }
    }
}

/// Inspect `_overrides` sub-packages for manual override of the generated code.
///
/// This is the hacky way. We extract all identifiers from `__init__.py` which contains, but don't
/// start with, a underscore (`_`).
fn load_overrides(path: &Utf8Path) -> HashSet<String> {
    let path = path.join("_overrides").join("__init__.py");
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("couldn't load overrides module at {path:?}"))
        .unwrap();

    // extract words from contents
    contents
        .split_whitespace()
        .filter(|word| !word.starts_with('_') && !word.starts_with('.') && word.contains('_'))
        .map(|word| word.trim_end_matches(',').to_owned())
        .collect()
}

impl CodeGenerator for PythonCodeGenerator {
    fn generate(&mut self, objs: &Objects, arrow_registry: &ArrowRegistry) -> Vec<Utf8PathBuf> {
        let mut filepaths = Vec::new();

        let datatypes_path = self.pkg_path.join("datatypes");
        let datatype_overrides = load_overrides(&datatypes_path);
        std::fs::create_dir_all(&datatypes_path)
            .with_context(|| format!("{datatypes_path:?}"))
            .unwrap();
        filepaths.extend(
            quote_objects(
                datatypes_path,
                arrow_registry,
                &datatype_overrides,
                objs,
                ObjectKind::Datatype,
                &objs.ordered_objects(ObjectKind::Datatype.into()),
            )
            .0,
        );

        let components_path = self.pkg_path.join("components");
        let component_overrides = load_overrides(&components_path);
        std::fs::create_dir_all(&components_path)
            .with_context(|| format!("{components_path:?}"))
            .unwrap();
        filepaths.extend(
            quote_objects(
                components_path,
                arrow_registry,
                &component_overrides,
                objs,
                ObjectKind::Component,
                &objs.ordered_objects(ObjectKind::Component.into()),
            )
            .0,
        );

        let archetypes_path = self.pkg_path.join("archetypes");
        let archetype_overrides = load_overrides(&archetypes_path);
        std::fs::create_dir_all(&archetypes_path)
            .with_context(|| format!("{archetypes_path:?}"))
            .unwrap();
        let (paths, archetype_names) = quote_objects(
            archetypes_path,
            arrow_registry,
            &archetype_overrides,
            objs,
            ObjectKind::Archetype,
            &objs.ordered_objects(ObjectKind::Archetype.into()),
        );
        filepaths.extend(paths);

        filepaths.push(quote_lib(&self.pkg_path, &archetype_names));

        filepaths
    }
}

// --- File management ---

fn quote_lib(out_path: impl AsRef<Utf8Path>, archetype_names: &[String]) -> Utf8PathBuf {
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
    out_path: impl AsRef<Utf8Path>,
    arrow_registry: &ArrowRegistry,
    overrides: &HashSet<String>,
    all_objects: &Objects,
    _kind: ObjectKind,
    objs: &[&Object],
) -> (Vec<Utf8PathBuf>, Vec<String>) {
    let out_path = out_path.as_ref();

    let mut filepaths = Vec::new();
    let mut all_names = Vec::new();

    let mut files = HashMap::<Utf8PathBuf, Vec<QuotedObject>>::new();
    for obj in objs {
        all_names.push(obj.name.clone());

        let obj = if obj.is_struct() {
            QuotedObject::from_struct(arrow_registry, overrides, all_objects, obj)
        } else {
            QuotedObject::from_union(arrow_registry, overrides, all_objects, obj)
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
            .flat_map(|obj| match obj.object.kind {
                ObjectKind::Datatype | ObjectKind::Component => {
                    let name = &obj.object.name;

                    if obj.object.is_delegating_component() {
                        vec![format!("{name}Array"), format!("{name}Type")]
                    } else {
                        vec![
                            format!("{name}"),
                            format!("{name}Like"),
                            format!("{name}Array"),
                            format!("{name}ArrayLike"),
                            format!("{name}Type"),
                        ]
                    }
                }
                ObjectKind::Archetype => vec![obj.object.name.clone()],
            })
            .collect::<Vec<_>>();

        // NOTE: Isolating the file stem only works because we're handling datatypes, components
        // and archetypes separately (and even then it's a bit shady, eh).
        mods.entry(filepath.file_stem().unwrap().to_owned())
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
            "
            from __future__ import annotations

            import numpy as np
            import numpy.typing as npt
            import pyarrow as pa

            from attrs import define, field
            from typing import Any, Dict, Iterable, Optional, Sequence, Set, Tuple, Union, TYPE_CHECKING

            from .._baseclasses import (
                Archetype,
                BaseExtensionType,
                BaseExtensionArray,
                BaseDelegatingExtensionType,
                BaseDelegatingExtensionArray
            )
            from .._converters import (
                to_np_uint8,
                to_np_uint16,
                to_np_uint32,
                to_np_uint64,
                to_np_int8,
                to_np_int16,
                to_np_int32,
                to_np_int64,
                to_np_bool,
                to_np_float16,
                to_np_float32,
                to_np_float64
            )
            ",
            0,
        );

        // import all overrides
        let override_names: Vec<_> = objs
            .iter()
            .flat_map(|obj| {
                let name = obj.object.name.as_str().to_lowercase();
                overrides
                    .iter()
                    .filter(|o| o.starts_with(name.as_str()))
                    .map(|o| o.as_str())
                    .collect::<Vec<_>>()
            })
            .collect();

        // TODO(ab): remove this noqa—useful for checking what overrides are extracted
        if !override_names.is_empty() {
            code.push_unindented_text(
                format!(
                    "
                    from ._overrides import {}  # noqa: F401
                    ",
                    override_names.join(", ")
                ),
                0,
            );
        }

        let import_clauses: HashSet<_> = objs
            .iter()
            .flat_map(|obj| obj.object.fields.iter())
            .filter_map(quote_import_clauses_from_field)
            .collect();
        for clause in import_clauses {
            code.push_text(&clause, 1, 0);
        }

        code.push_unindented_text(
            format!(
                "
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
            "
            from __future__ import annotations

            ",
            0,
        );

        for (module, names) in &mods {
            let names = names.join(", ");
            code.push_text(&format!("from .{module} import {names}"), 1, 0);
        }

        code.push_unindented_text(format!("\n__all__ = [{manifest}]"), 0);

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
    object: Object,
    filepath: Utf8PathBuf,
    code: String,
}

impl QuotedObject {
    fn from_struct(
        arrow_registry: &ArrowRegistry,
        overrides: &HashSet<String>,
        objects: &Objects,
        obj: &Object,
    ) -> Self {
        assert!(obj.is_struct());

        let Object {
            virtpath,
            filepath: _,
            fqname: _,
            pkg_name: _,
            name,
            docs,
            kind,
            attrs: _,
            order: _,
            fields,
            specifics: _,
            datatype: _,
        } = obj;

        let mut code = String::new();

        if *kind != ObjectKind::Component || obj.is_non_delegating_component() {
            let superclass = match *kind {
                ObjectKind::Archetype => "(Archetype)",
                ObjectKind::Component | ObjectKind::Datatype => "",
            };

            let define_args = if *kind == ObjectKind::Archetype {
                "(str=False, repr=False)"
            } else {
                ""
            };

            code.push_unindented_text(
                format!(
                    r#"

                @define{define_args}
                class {name}{superclass}:
                "#
                ),
                0,
            );

            code.push_text(quote_doc_from_docs(docs), 0, 4);

            // NOTE: We need to add required fields first, and then optional ones, otherwise mypy
            // complains.
            let fields_in_order = fields
                .iter()
                .filter(|field| !field.is_nullable)
                .chain(fields.iter().filter(|field| field.is_nullable));
            for field in fields_in_order {
                let ObjectField {
                    virtpath: _,
                    filepath: _,
                    fqname: _,
                    pkg_name: _,
                    name,
                    docs,
                    typ: _,
                    attrs: _,
                    order: _,
                    is_nullable,
                    is_deprecated: _,
                    datatype: _,
                } = field;

                let (typ, _, default_converter) =
                    quote_field_type_from_field(objects, field, false);
                let (typ_unwrapped, _, _) = quote_field_type_from_field(objects, field, true);
                let typ = if *kind == ObjectKind::Archetype {
                    format!("{typ_unwrapped}Array")
                } else {
                    typ
                };

                let converter = if *kind == ObjectKind::Archetype {
                    // archetype always delegate field init to the component array object
                    format!("converter={typ_unwrapped}Array.from_similar, # type: ignore[misc]\n")
                } else {
                    // components and datatypes have converters only if manually provided
                    let override_name = format!(
                        "{}_{}_converter",
                        obj.name.to_lowercase(),
                        name.to_lowercase()
                    );
                    if overrides.contains(&override_name) {
                        format!("converter={override_name}")
                    } else if !default_converter.is_empty() {
                        format!("converter={default_converter}")
                    } else {
                        String::new()
                    }
                };

                let metadata = if *kind == ObjectKind::Archetype {
                    format!(
                        "\nmetadata={{'component': '{}'}}, ",
                        if *is_nullable { "secondary" } else { "primary" }
                    )
                } else {
                    String::new()
                };

                let typ = if !*is_nullable {
                    format!("{typ} = field({metadata}{converter})")
                } else {
                    format!(
                        "{typ} | None = field({metadata}default=None{}{converter})",
                        if converter.is_empty() { "" } else { ", " },
                    )
                };

                code.push_text(format!("{name}: {typ}"), 1, 4);

                code.push_text(quote_doc_from_docs(docs), 0, 4);
            }

            if *kind == ObjectKind::Archetype {
                code.push_text("__str__ = Archetype.__str__", 1, 4);
                code.push_text("__repr__ = Archetype.__repr__", 1, 4);
            }

            code.push_text(quote_array_method_from_obj(overrides, objects, obj), 1, 4);
            code.push_text(quote_native_types_method_from_obj(objects, obj), 1, 4);

            if *kind != ObjectKind::Archetype {
                code.push_text(quote_aliases_from_object(obj), 1, 0);
            }
        }

        match kind {
            ObjectKind::Archetype => (),
            ObjectKind::Component => {
                // a component might be either delegating to a datatype or using a native type
                if let Type::Object(ref dtype_fqname) = obj.fields[0].typ {
                    let dtype_obj = objects.get(dtype_fqname);
                    code.push_text(
                        quote_arrow_support_from_delegating_component(obj, dtype_obj),
                        1,
                        0,
                    );
                } else {
                    code.push_text(
                        quote_arrow_support_from_obj(arrow_registry, overrides, obj),
                        1,
                        0,
                    );
                }
            }
            ObjectKind::Datatype => {
                code.push_text(
                    quote_arrow_support_from_obj(arrow_registry, overrides, obj),
                    1,
                    0,
                );
            }
        }

        let mut filepath = Utf8PathBuf::from(virtpath);
        filepath.set_extension("py");

        Self {
            object: obj.clone(),
            filepath,
            code,
        }
    }

    // TODO(ab): this function is likely broken, to be handled in next PR
    fn from_union(
        arrow_registry: &ArrowRegistry,
        overrides: &HashSet<String>,
        objects: &Objects,
        obj: &Object,
    ) -> Self {
        assert!(!obj.is_struct());
        assert_eq!(obj.kind, ObjectKind::Datatype);

        let Object {
            virtpath,
            filepath: _,
            fqname: _,
            pkg_name: _,
            name,
            docs,
            kind,
            attrs: _,
            order: _,
            fields,
            specifics: _,
            datatype: _,
        } = obj;

        let mut code = String::new();

        code.push_unindented_text(
            format!(
                r#"

                @define
                class {name}:
                "#
            ),
            0,
        );

        code.push_text(quote_doc_from_docs(docs), 0, 4);

        for field in fields {
            let ObjectField {
                virtpath: _,
                filepath: _,
                fqname: _,
                pkg_name: _,
                name,
                docs,
                typ: _,
                attrs: _,
                order: _,
                is_nullable: _,
                is_deprecated: _,
                datatype: _,
            } = field;

            let (typ, _, _) = quote_field_type_from_field(objects, field, false);
            // NOTE: It's always optional since only one of the fields can be set at a time.
            code.push_text(format!("{name}: {typ} | None = None"), 1, 4);

            code.push_text(quote_doc_from_docs(docs), 0, 4);
        }

        code.push_text(quote_str_repr_from_obj(obj), 1, 4);
        code.push_text(quote_array_method_from_obj(overrides, objects, obj), 1, 4);
        code.push_text(quote_native_types_method_from_obj(objects, obj), 1, 4);

        code.push_text(quote_aliases_from_object(obj), 1, 0);
        match kind {
            ObjectKind::Archetype => (),
            ObjectKind::Component => {
                unreachable!("component may not be a union")
            }
            ObjectKind::Datatype => {
                code.push_text(
                    quote_arrow_support_from_obj(arrow_registry, overrides, obj),
                    1,
                    0,
                );
            }
        }

        let mut filepath = Utf8PathBuf::from(virtpath);
        filepath.set_extension("py");

        Self {
            object: obj.clone(),
            filepath,
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

fn quote_doc_from_docs(docs: &Docs) -> String {
    let lines = crate::codegen::get_documentation(docs, &["py", "python"]);

    if lines.is_empty() {
        return String::new();
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise...
    let doc = lines
        .into_iter()
        .filter(|line| !line.starts_with(r#"""""#))
        .collect_vec()
        .join("\n");

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
            for fld in fields(self):
                if "component" in fld.metadata:
                    comp: components.Component = getattr(self, fld.name)
                    if datatype := getattr(comp, "type"):
                        name = comp.extension_name
                        typ = datatype.storage_type
                        s += f"  {name}<{typ}>(\n    {comp.to_pylist()}\n  )\n"

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
fn quote_array_method_from_obj(
    overrides: &HashSet<String>,
    objects: &Objects,
    obj: &Object,
) -> String {
    // TODO(cmc): should be using the native type, but need to compare numpy types etc
    let typ = quote_field_type_from_field(objects, &obj.fields[0], false).0;

    // allow overriding the __array__ function
    let override_name = format!("{}_as_array", obj.name.to_lowercase());
    if overrides.contains(&override_name) {
        return unindent::unindent(&format!(
            "
            def __array__(self, dtype: npt.DTypeLike=None) -> npt.ArrayLike:
                return {override_name}(self, dtype=dtype)
            "
        ));
    }

    // exclude archetypes, objects which dont have a single field, and anything that isn't an numpy
    // array or scalar numbers
    if obj.kind == ObjectKind::Archetype
        || obj.fields.len() != 1
        || (!["npt.ArrayLike", "float", "int"].contains(&typ.as_str())
            && !typ.contains("npt.NDArray"))
    {
        return String::new();
    }

    let field_name = &obj.fields[0].name;
    unindent::unindent(&format!(
        "
        def __array__(self, dtype: npt.DTypeLike=None) -> npt.ArrayLike:
            return np.asarray(self.{field_name}, dtype=dtype)
        ",
    ))
}

/// Automatically implement `__str__`, `__int__`, or `__float__` method if the object has a single
/// field of the corresponding type that is not optional.
///
/// Only applies to datatypes and components.
fn quote_native_types_method_from_obj(objects: &Objects, obj: &Object) -> String {
    let typ = quote_field_type_from_field(objects, &obj.fields[0], false).0;
    let typ = typ.as_str();
    if
    // cannot be an archetype
    obj.kind == ObjectKind::Archetype
        // has to have a single field
        || obj.fields.len() != 1
        // that field cannot be optional
        || obj.fields[0].is_nullable
        // that single field must be of a supported native type
        // TODO(cmc): should be using the native type, but need to compare numpy types etc
        || !["str", "int", "float"].contains(&typ)
    {
        return String::new();
    }

    let field_name = &obj.fields[0].name;
    unindent::unindent(&format!(
        "
        def __{typ}__(self) -> {typ}:
            return {typ}(self.{field_name})
        ",
    ))
}

/// Only applies to datatypes and components.
fn quote_aliases_from_object(obj: &Object) -> String {
    assert_ne!(obj.kind, ObjectKind::Archetype);

    let aliases = obj.try_get_attr::<String>(ATTR_PYTHON_ALIASES);
    let array_aliases = obj
        .try_get_attr::<String>(ATTR_PYTHON_ARRAY_ALIASES)
        .unwrap_or_default();

    let name = &obj.name;

    let mut code = String::new();

    code.push_unindented_text("if TYPE_CHECKING:", 1);

    code.push_text(
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
        4,
    );

    code.push_text(
        format!(
            r#"
{name}ArrayLike = Union[
    {name},
    Sequence[{name}Like],
    {array_aliases}
]
"#,
        ),
        0,
        4,
    );

    code.push_unindented_text(
        format!(
            r#"
        else:
            {name}Like = Any
            {name}ArrayLike = Any
        "#
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
        let fqname = fqname.replace(".testing", "");
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
/// The third returned value is a default converter function if any.
fn quote_field_type_from_field(
    _objects: &Objects,
    field: &ObjectField,
    unwrap: bool,
) -> (String, bool, String) {
    let mut unwrapped = false;
    let mut converter = String::new();
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
            match elem_type {
                ElementType::UInt8 => converter = "to_np_uint8".to_owned(),
                ElementType::UInt16 => converter = "to_np_uint16".to_owned(),
                ElementType::UInt32 => converter = "to_np_uint32".to_owned(),
                ElementType::UInt64 => converter = "to_np_uint64".to_owned(),
                ElementType::Int8 => converter = "to_np_int8".to_owned(),
                ElementType::Int16 => converter = "to_np_int16".to_owned(),
                ElementType::Int32 => converter = "to_np_int32".to_owned(),
                ElementType::Int64 => converter = "to_np_int64".to_owned(),
                ElementType::Bool => converter = "to_np_bool".to_owned(),
                ElementType::Float16 => converter = "to_np_float16".to_owned(),
                ElementType::Float32 => converter = "to_np_float32".to_owned(),
                ElementType::Float64 => converter = "to_np_float64".to_owned(),
                _ => {}
            };

            match elem_type {
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
                ElementType::String => "list[str]".to_owned(),
                ElementType::Object(_) => {
                    let typ = quote_type_from_element_type(elem_type);
                    if unwrap {
                        unwrapped = true;
                        typ
                    } else {
                        format!("list[{typ}]")
                    }
                }
            }
        }
        Type::Object(fqname) => quote_type_from_element_type(&ElementType::Object(fqname.clone())),
    };

    (typ, unwrapped, converter)
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
            let fqname = fqname.replace(".testing", "");
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

fn quote_arrow_support_from_delegating_component(obj: &Object, dtype_obj: &Object) -> String {
    let Object {
        fqname, name, kind, ..
    } = obj;

    assert_eq!(
        *kind,
        ObjectKind::Component,
        "this function only handles components"
    );

    // extract datatype, must be *only* one
    assert_eq!(
        obj.fields.len(),
        1,
        "component must have exactly one field, but {} has {}",
        fqname,
        obj.fields.len()
    );

    let extension_type = format!("{name}Type");
    let extension_array = format!("{name}Array");

    let dtype_extension_type = format!("{}Type", dtype_obj.name);
    let dtype_extension_array = format!("{}Array", dtype_obj.name);
    let dtype_extension_array_like = format!("{}ArrayLike", dtype_obj.name);

    let legacy_fqname = obj
        .try_get_attr::<String>(ATTR_RERUN_LEGACY_FQNAME)
        .unwrap_or_else(|| fqname.clone());

    unindent::unindent(&format!(
        r#"

        class {extension_type}(BaseDelegatingExtensionType):
            _TYPE_NAME = "{legacy_fqname}"
            _DELEGATED_EXTENSION_TYPE = datatypes.{dtype_extension_type}

        class {extension_array}(BaseDelegatingExtensionArray[datatypes.{dtype_extension_array_like}]):
            _EXTENSION_NAME = "{legacy_fqname}"
            _EXTENSION_TYPE = {extension_type}
            _DELEGATED_ARRAY_TYPE = datatypes.{dtype_extension_array}

        {extension_type}._ARRAY_TYPE = {extension_array}

        # TODO(cmc): bring back registration to pyarrow once legacy types are gone
        # pa.register_extension_type({extension_type}())
        "#
    ))
}

/// Arrow support objects
///
/// Generated for Components using native types and Datatypes. Components using a Datatype instead
/// delegate to the Datatype's arrow support.
fn quote_arrow_support_from_obj(
    arrow_registry: &ArrowRegistry,
    overrides: &HashSet<String>,
    obj: &Object,
) -> String {
    let Object { fqname, name, .. } = obj;

    let (ext_type_base, ext_array_base) =
        if obj.kind == ObjectKind::Datatype || obj.is_non_delegating_component() {
            ("BaseExtensionType", "BaseExtensionArray")
        } else if obj.is_delegating_component() {
            (
                "BaseDelegatingExtensionType",
                "BaseDelegatingExtensionArray",
            )
        } else {
            unreachable!("archetypes do not have arrow support")
        };

    let datatype = quote_arrow_datatype(&arrow_registry.get(fqname));
    let extension_array = format!("{name}Array");
    let extension_type = format!("{name}Type");
    let many_aliases = format!("{name}ArrayLike");

    let legacy_fqname = obj
        .try_get_attr::<String>(ATTR_RERUN_LEGACY_FQNAME)
        .unwrap_or_else(|| fqname.clone());

    let name_lower = name.to_lowercase();
    let override_name = format!("{name_lower}_native_to_pa_array");
    let override_ = if overrides.contains(&override_name) {
        format!("return {name_lower}_native_to_pa_array(data, data_type)")
    } else {
        "raise NotImplementedError".to_owned()
    };

    unindent::unindent(&format!(
        r#"

        # --- Arrow support ---

        class {extension_type}({ext_type_base}):
            def __init__(self) -> None:
                pa.ExtensionType.__init__(
                    self, {datatype}, "{legacy_fqname}"
                )

        class {extension_array}({ext_array_base}[{many_aliases}]):
            _EXTENSION_NAME = "{legacy_fqname}"
            _EXTENSION_TYPE = {extension_type}

            @staticmethod
            def _native_to_pa_array(data: {many_aliases}, data_type: pa.DataType) -> pa.Array:
                {override_}

        {extension_type}._ARRAY_TYPE = {extension_array}

        # TODO(cmc): bring back registration to pyarrow once legacy types are gone
        # pa.register_extension_type({extension_type}())
        "#
    ))
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

        DataType::List(field) => {
            let field = quote_arrow_field(field);
            format!("pa.list_({field})")
        }

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
