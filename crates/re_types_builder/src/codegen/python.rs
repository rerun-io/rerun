//! Implements the Python codegen pass.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    io::Write,
};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use rayon::prelude::*;

use crate::{
    codegen::{autogen_warning, StringExt as _},
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
                    Some(&objects[name])
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
/// Returns function names.
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
    fn generate(
        &mut self,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
    ) -> BTreeSet<Utf8PathBuf> {
        let mut files_to_write: BTreeMap<Utf8PathBuf, String> = Default::default();

        for object_kind in ObjectKind::ALL {
            self.generate_folder(objects, arrow_registry, object_kind, &mut files_to_write);
        }

        {
            let archetype_names = objects
                .ordered_objects(ObjectKind::Archetype.into())
                .iter()
                .map(|o| o.name.clone())
                .collect_vec();
            files_to_write.insert(
                self.pkg_path.join("__init__.py"),
                lib_source_code(&archetype_names),
            );
        }

        write_files(&files_to_write);

        let filepaths = files_to_write.keys().cloned().collect();

        for kind in ObjectKind::ALL {
            let folder_path = self.pkg_path.join(kind.plural_snake_case());
            super::common::remove_old_files_from_folder(folder_path, &filepaths);
        }

        filepaths
    }
}

impl PythonCodeGenerator {
    fn generate_folder(
        &self,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
        object_kind: ObjectKind,
        files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
    ) {
        let kind_path = self.pkg_path.join(object_kind.plural_snake_case());
        let overrides = load_overrides(&kind_path);

        // (module_name, [object_name])
        let mut mods = HashMap::<String, Vec<String>>::new();

        // Generate folder contents:
        let ordered_objects = objects.ordered_objects(object_kind.into());
        for &obj in &ordered_objects {
            let filepath = kind_path.join(format!("{}.py", obj.snake_case_name()));

            let names = match obj.kind {
                ObjectKind::Datatype | ObjectKind::Component => {
                    let name = &obj.name;

                    if obj.is_delegating_component() {
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
                ObjectKind::Archetype => vec![obj.name.clone()],
            };

            // NOTE: Isolating the file stem only works because we're handling datatypes, components
            // and archetypes separately (and even then it's a bit shady, eh).
            mods.entry(filepath.file_stem().unwrap().to_owned())
                .or_default()
                .extend(names.iter().cloned());

            let mut code = String::new();
            code.push_text(&format!("# {}", autogen_warning!()), 1, 0);
            if let Some(source_path) = obj.relative_filepath() {
                code.push_text(&format!("# Based on {source_path:?}.\n\n"), 2, 0);
            }

            let manifest = quote_manifest(names);

            code.push_unindented_text(
                "
            from __future__ import annotations

            from typing import (Any, Dict, Iterable, Optional, Sequence, Set, Tuple, Union,
                TYPE_CHECKING, SupportsFloat, Literal)

            from attrs import define, field
            import numpy as np
            import numpy.typing as npt
            import pyarrow as pa
            import uuid

            from .._baseclasses import (
                Archetype,
                BaseExtensionType,
                BaseExtensionArray,
                BaseDelegatingExtensionType,
                BaseDelegatingExtensionArray
            )
            from .._converters import (
                int_or_none,
                float_or_none,
                bool_or_none,
                str_or_none,
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
            let obj_override_prefix = format!("override_{}", obj.snake_case_name());
            let override_names: Vec<_> = overrides
                .iter()
                .filter(|o| o.starts_with(&obj_override_prefix))
                .map(|o| o.as_str())
                .collect::<Vec<_>>();

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

            let import_clauses: HashSet<_> = obj
                .fields
                .iter()
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

            let obj_code = if obj.is_struct() {
                code_for_struct(arrow_registry, &overrides, objects, obj)
            } else {
                code_for_union(arrow_registry, &overrides, objects, obj)
            };
            code.push_text(&obj_code, 1, 0);

            files_to_write.insert(filepath.clone(), code);
        }

        // rerun/{datatypes|components|archetypes}/__init__.py
        {
            let path = kind_path.join("__init__.py");

            let mut code = String::new();

            let manifest = quote_manifest(mods.iter().flat_map(|(_, names)| names.iter()));

            code.push_text(&format!("# {}", autogen_warning!()), 2, 0);
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

            files_to_write.insert(path, code);
        }
    }
}

fn write_files(files_to_write: &BTreeMap<Utf8PathBuf, String>) {
    re_tracing::profile_function!();
    // TODO(emilk): running `black` and `ruff` once for each file is very slow.
    // It would probably be faster to write all files to a temporary folder, run `black` and `ruff` on
    // that folder, and then copy the results to the final destination (if the files has changed).
    files_to_write.par_iter().for_each(|(path, source)| {
        write_file(path, source.clone());
    });
}

fn write_file(filepath: &Utf8PathBuf, mut source: String) {
    re_tracing::profile_function!();

    match format_python(&source) {
        Ok(formatted) => source = formatted,
        Err(err) => {
            // NOTE: Formatting code requires both `black` and `ruff` to be in $PATH, but only for contributors,
            // not end users.
            // Even for contributors, `black` and `ruff` won't be needed unless they edit some of the
            // .fbs files… and even then, this won't crash if they are missing, it will just fail to pass
            // the CI!
            re_log::warn_once!(
                "Failed to format Python code: {err}. Make sure `black` and `ruff` are installed."
            );
        }
    }

    super::common::write_file(filepath, source);
}

fn lib_source_code(archetype_names: &[String]) -> String {
    let manifest = quote_manifest(archetype_names);
    let archetype_names = archetype_names.join(", ");

    let mut code = String::new();

    code += &unindent::unindent(&format!(
        r#"
        # {autogen_warning}

        from __future__ import annotations

        __all__ = [{manifest}]

        from .archetypes import {archetype_names}
        "#,
        autogen_warning = autogen_warning!()
    ));

    code
}

// --- Codegen core loop ---

fn code_for_struct(
    arrow_registry: &ArrowRegistry,
    overrides: &HashSet<String>,
    objects: &Objects,
    obj: &Object,
) -> String {
    assert!(obj.is_struct());

    let Object {
        name,
        docs,
        kind,
        fields,
        ..
    } = obj;

    let mut code = String::new();

    if *kind != ObjectKind::Component || obj.is_non_delegating_component() {
        // field converters preprocessing pass — must be performed here because we must autogen
        // converter function *before* the class
        let mut field_converters: HashMap<String, String> = HashMap::new();
        for field in fields {
            let (default_converter, converter_function) =
                quote_field_converter_from_field(obj, objects, field);

            let converter_override_name = format!(
                "override_{}_{}_converter",
                obj.snake_case_name(),
                field.name
            );
            let converter = if overrides.contains(&converter_override_name) {
                format!("converter={converter_override_name}")
            } else if *kind == ObjectKind::Archetype {
                // Archetypes default to using `from_similar` from the Component
                let (typ_unwrapped, _) = quote_field_type_from_field(objects, field, true);
                // archetype always delegate field init to the component array object
                format!("converter={typ_unwrapped}Array.from_similar, # type: ignore[misc]\n")
            } else if !default_converter.is_empty() {
                code.push_text(&converter_function, 1, 0);
                format!("converter={default_converter}")
            } else {
                String::new()
            };
            field_converters.insert(field.fqname.clone(), converter);
        }

        // init override handling
        let init_override_name = format!("override_{}_init", obj.snake_case_name());
        let (init_define_arg, init_func) = if overrides.contains(&init_override_name) {
            ("init=False".to_owned(), init_override_name)
        } else {
            (String::new(), String::new())
        };

        let superclass = match *kind {
            ObjectKind::Archetype => "(Archetype)",
            ObjectKind::Component | ObjectKind::Datatype => "",
        };

        let define_args = if *kind == ObjectKind::Archetype {
            format!(
                "str=False, repr=False{}{init_define_arg}",
                if init_define_arg.is_empty() { "" } else { ", " }
            )
        } else {
            init_define_arg
        };

        let define_args = if !define_args.is_empty() {
            format!("({define_args})")
        } else {
            define_args
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

        if init_func.is_empty() {
            code.push_text("# You can define your own __init__ function by defining a function called {init_override_name:?}", 2, 4);
        } else {
            code.push_text(
                "def __init__(self, *args, **kwargs):  #type: ignore[no-untyped-def]",
                1,
                4,
            );
            code.push_text(format!("{init_func}(self, *args, **kwargs)"), 2, 8);
        }

        // NOTE: We need to add required fields first, and then optional ones, otherwise mypy
        // complains.
        // TODO(ab, #2641): this is required because fields without default should appear before fields
        //  with default. Now, `TranslationXXX.from_parent` *should* have a default value,
        //  and appear at the end of the list, but it currently doesn't. This is unfortunate as
        //  the apparent field order is inconsistent with what the `xxxx_init()` override
        //  accepts.
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

            let (typ, _) = quote_field_type_from_field(objects, field, false);
            let (typ_unwrapped, _) = quote_field_type_from_field(objects, field, true);
            let typ = if *kind == ObjectKind::Archetype {
                format!("{typ_unwrapped}Array")
            } else {
                typ
            };

            let metadata = if *kind == ObjectKind::Archetype {
                format!(
                    "\nmetadata={{'component': '{}'}}, ",
                    if *is_nullable { "secondary" } else { "primary" }
                )
            } else {
                String::new()
            };

            let converter = &field_converters[&field.fqname];
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
                let dtype_obj = &objects[dtype_fqname];
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

    code
}

fn code_for_union(
    arrow_registry: &ArrowRegistry,
    overrides: &HashSet<String>,
    objects: &Objects,
    obj: &Object,
) -> String {
    assert!(!obj.is_struct());
    assert_eq!(obj.kind, ObjectKind::Datatype);

    let Object {
        name,
        docs,
        kind,
        fields,
        ..
    } = obj;

    let mut code = String::new();

    // init override handling
    let init_override_name = format!("override_{}_init", obj.snake_case_name());
    let (define_args, init_func) = if overrides.contains(&init_override_name) {
        ("(init=False)".to_owned(), init_override_name)
    } else {
        (String::new(), String::new())
    };

    code.push_unindented_text(
        format!(
            r#"

                @define{define_args}
                class {name}:
                "#
        ),
        0,
    );

    code.push_text(quote_doc_from_docs(docs), 0, 4);

    if init_func.is_empty() {
        code.push_text("# You can define your own __init__ function by defining a function called {init_override_name:?}", 2, 4);
    } else {
        code.push_text(
            "def __init__(self, *args, **kwargs):  #type: ignore[no-untyped-def]",
            1,
            4,
        );
        code.push_text(format!("{init_func}(self, *args, **kwargs)"), 2, 8);
    }

    let field_types = fields
        .iter()
        .map(|f| quote_field_type_from_field(objects, f, false).0)
        .collect::<BTreeSet<_>>();
    let has_duplicate_types = field_types.len() != fields.len();

    // provide a default converter if *all* arms are of the same type
    let default_converter = if field_types.len() == 1 {
        quote_field_converter_from_field(obj, objects, &fields[0]).0
    } else {
        String::new()
    };

    let inner_type = if field_types.len() > 1 {
        format!("Union[{}]", field_types.iter().join(", "))
    } else {
        field_types.iter().next().unwrap().to_string()
    };

    // components and datatypes have converters only if manually provided
    let converter_override_name = format!("override_{}_inner_converter", obj.snake_case_name());
    let converter = if overrides.contains(&converter_override_name) {
        format!("converter={converter_override_name}")
    } else if !default_converter.is_empty() {
        format!("converter={default_converter}")
    } else {
        String::new()
    };

    code.push_text(format!("inner: {inner_type} = field({converter})"), 1, 4);
    code.push_text(quote_doc_from_fields(objects, fields), 0, 4);

    // if there are duplicate types, we need to add a `kind` field to disambiguate the union
    if has_duplicate_types {
        let kind_type = fields
            .iter()
            .map(|f| format!("{:?}", f.name.to_lowercase()))
            .join(", ");
        let first_kind = &fields[0].name.to_lowercase();

        code.push_text(
            format!("kind: Literal[{kind_type}] = field(default={first_kind:?})"),
            1,
            4,
        );
    }

    code.push_unindented_text(quote_union_aliases_from_object(obj, field_types.iter()), 1);

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

    code
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

fn quote_doc_from_fields(objects: &Objects, fields: &Vec<ObjectField>) -> String {
    let mut lines = vec![];

    for field in fields {
        let field_lines = crate::codegen::get_documentation(&field.docs, &["py", "python"]);
        lines.push(format!(
            "{} ({}):",
            field.name,
            quote_field_type_from_field(objects, field, false).0
        ));
        lines.extend(field_lines.into_iter().map(|line| format!("    {line}")));
        lines.push(String::new());
    }

    if lines.is_empty() {
        return String::new();
    } else {
        // remove last empty line
        lines.pop();
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise...
    let doc = lines
        .into_iter()
        .filter(|line| !line.starts_with(r#"""""#))
        .collect_vec()
        .join("\n");

    format!("\"\"\"\n{doc}\n\"\"\"\n\n")
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
    let override_name = format!("override_{}_as_array", obj.snake_case_name());
    if overrides.contains(&override_name) {
        return unindent::unindent(&format!(
            "
            def __array__(self, dtype: npt.DTypeLike=None) -> npt.NDArray[Any]:
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
        def __array__(self, dtype: npt.DTypeLike=None) -> npt.NDArray[Any]:
            # You can replace `np.asarray` here with your own code by defining a function named {override_name:?}
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

    code.push_unindented_text(
        &if let Some(aliases) = aliases {
            format!(
                r#"
                if TYPE_CHECKING:
                    {name}Like = Union[
                        {name},
                        {aliases}
                    ]
                else:
                    {name}Like = Any
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
                {name},
                Sequence[{name}Like],
                {array_aliases}
            ]
            "#,
        ),
        0,
    );

    code
}

/// Quote typing aliases for union datatypes. The types for the union arms are automatically
/// included.
fn quote_union_aliases_from_object<'a>(
    obj: &Object,
    mut field_types: impl Iterator<Item = &'a String>,
) -> String {
    assert_ne!(obj.kind, ObjectKind::Archetype);

    let aliases = obj.try_get_attr::<String>(ATTR_PYTHON_ALIASES);
    let array_aliases = obj
        .try_get_attr::<String>(ATTR_PYTHON_ARRAY_ALIASES)
        .unwrap_or_default();

    let name = &obj.name;

    let union_fields = field_types.join(",");
    let aliases = if let Some(aliases) = aliases {
        aliases
    } else {
        String::new()
    };

    unindent::unindent(&format!(
        r#"
            if TYPE_CHECKING:
                {name}Like = Union[
                    {name},{union_fields},{aliases}
                ]
                {name}ArrayLike = Union[
                    {name},{union_fields},
                    Sequence[{name}Like],{array_aliases}
                ]
            else:
                {name}Like = Any
                {name}ArrayLike = Any
            "#,
    ))
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
            // NOTE: This is assuming importing other archetypes is legal… which whether it is or
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
    _objects: &Objects,
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
        },
        Type::Object(fqname) => quote_type_from_element_type(&ElementType::Object(fqname.clone())),
    };

    (typ, unwrapped)
}

/// Returns a default converter function for the given field.
///
/// Returns the converter name and, if needed, the converter function itself.
fn quote_field_converter_from_field(
    obj: &Object,
    objects: &Objects,
    field: &ObjectField,
) -> (String, String) {
    let mut function = String::new();

    let converter = match &field.typ {
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
        Type::Object(fqname) => {
            let typ = quote_type_from_element_type(&ElementType::Object(fqname.clone()));
            let field_obj = &objects[fqname];

            // we generate a default converter only if the field's type can be constructed with a
            // single argument
            if field_obj.fields.len() == 1 || field_obj.is_union() {
                let converter_name = format!(
                    "_override_{}_{}_converter",
                    obj.snake_case_name(),
                    field.name
                );

                // generate the converter function
                if field.is_nullable {
                    function.push_unindented_text(
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
                    function.push_unindented_text(
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
                // NOTE: This is assuming importing other archetypes is legal… which whether it is or
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

    let override_name = format!("override_{}_native_to_pa_array_override", obj.snake_case_name());
    let override_ = if overrides.contains(&override_name) {
        format!("return {override_name}(data, data_type)")
    } else {
        let override_file_path = format!(
            "rerun_py/rerun_sdk/rerun/_rerun2/{}/_overrides/{}.py",
            obj.kind.plural_snake_case(),
            obj.snake_case_name()
        );
        format!("raise NotImplementedError # You need to implement {override_name:?} in {override_file_path}")
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

        _ => unimplemented!("{datatype:#?}"),
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

    format!(r#"pa.field("{name}", {datatype}, nullable={is_nullable}, metadata={metadata})"#)
}

fn quote_metadata_map(metadata: &BTreeMap<String, String>) -> String {
    let kvs = metadata
        .iter()
        .map(|(k, v)| format!("{k:?}, {v:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{kvs}}}")
}

fn format_python(source: &str) -> anyhow::Result<String> {
    re_tracing::profile_function!();

    // The order below is important and sadly we need to call black twice. Ruff does not yet
    // fix line-length (See: https://github.com/astral-sh/ruff/issues/1904).
    //
    // 1) Call black, which among others things fixes line-length
    // 2) Call ruff, which requires line-lengths to be correct
    // 3) Call black again to cleanup some whitespace issues ruff might introduce

    let mut source = run_black(source).context("black")?;
    source = run_ruff(&source).context("ruff")?;
    source = run_black(&source).context("black")?;
    Ok(source)
}

fn python_project_path() -> Utf8PathBuf {
    let path = crate::rerun_workspace_path()
        .join("rerun_py")
        .join("pyproject.toml");
    assert!(path.exists(), "Failed to find {path:?}");
    path
}

fn run_black(source: &str) -> anyhow::Result<String> {
    re_tracing::profile_function!();
    use std::process::{Command, Stdio};

    let mut proc = Command::new("black")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg(format!("--config={}", python_project_path()))
        .arg("-") // Read from stdin
        .spawn()?;

    {
        let mut stdin = proc.stdin.take().unwrap();
        stdin.write_all(source.as_bytes())?;
    }

    let output = proc.wait_with_output()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout)
    } else {
        let stderr = String::from_utf8(output.stderr)?;
        anyhow::bail!("{stderr}")
    }
}

fn run_ruff(source: &str) -> anyhow::Result<String> {
    re_tracing::profile_function!();
    use std::process::{Command, Stdio};

    let mut proc = Command::new("ruff")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg(format!("--config={}", python_project_path()))
        .arg("--fix")
        .arg("-") // Read from stdin
        .spawn()?;

    {
        let mut stdin = proc.stdin.take().unwrap();
        stdin.write_all(source.as_bytes())?;
    }

    let output = proc.wait_with_output()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(stdout)
    } else {
        let stderr = String::from_utf8(output.stderr)?;
        anyhow::bail!("{stderr}")
    }
}
