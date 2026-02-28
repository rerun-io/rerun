//! Implements the Python codegen pass.

mod views;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::iter;
use std::ops::Deref;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::{Itertools as _, chain};
use regex_lite::Regex;
use unindent::unindent;

use self::views::code_for_view;
use super::Target;
use super::common::ExampleInfo;
use crate::codegen::common::{Example, collect_snippets_for_api_docs};
use crate::codegen::{StringExt as _, autogen_warning};
use crate::data_type::{AtomicDataType, DataType, Field, UnionMode};
use crate::objects::{ObjectClass, State};
use crate::{
    ATTR_PYTHON_ALIASES, ATTR_PYTHON_ARRAY_ALIASES, CodeGenerator, Docs, ElementType,
    GeneratedFiles, Object, ObjectField, ObjectKind, Objects, Reporter, Type, TypeRegistry,
    format_path,
};

/// The standard python init method.
const INIT_METHOD: &str = "__init__";

/// The standard numpy interface for converting to an array type
const ARRAY_METHOD: &str = "__array__";

/// The standard python len method
const LEN_METHOD: &str = "__len__";

/// The method used to convert a native type into a pyarrow array
const NATIVE_TO_PA_ARRAY_METHOD: &str = "native_to_pa_array_override";

/// The method used for deferred patch class init.
/// Use this for initialization constants that need to know the child (non-extension) class.
const DEFERRED_PATCH_CLASS_METHOD: &str = "deferred_patch_class";

/// The common suffix for method used to convert fields to their canonical representation.
const FIELD_CONVERTER_SUFFIX: &str = "__field_converter_override";

// ---

fn classmethod_decorators(obj: &Object) -> String {
    // We need to decorate all class methods as deprecated
    if let Some(deprecation_summary) = obj.deprecation_summary() {
        format!(r#"@deprecated("""{deprecation_summary}""")"#)
    } else {
        Default::default()
    }
}

// ---

/// Python-specific helpers for [`Object`].
trait PythonObjectExt {
    /// Returns `true` if the object is a delegating component.
    ///
    /// Components can either use a native type, or a custom datatype. In the latter case, the
    /// component delegates its implementation to the datatype.
    fn is_delegating_component(&self) -> bool;

    /// Returns `true` if the object is a non-delegating component.
    fn is_non_delegating_component(&self) -> bool;

    /// If the object is a delegating component, returns the datatype it delegates to.
    fn delegate_datatype<'a>(&self, objects: &'a Objects) -> Option<&'a Object>;
}

impl PythonObjectExt for Object {
    fn is_delegating_component(&self) -> bool {
        self.kind == ObjectKind::Component && matches!(self.fields[0].typ, Type::Object { .. })
    }

    fn is_non_delegating_component(&self) -> bool {
        self.kind == ObjectKind::Component && !self.is_delegating_component()
    }

    fn delegate_datatype<'a>(&self, objects: &'a Objects) -> Option<&'a Object> {
        self.is_delegating_component()
            .then(|| {
                if let Type::Object { fqname } = &self.fields[0].typ {
                    Some(&objects[fqname])
                } else {
                    None
                }
            })
            .flatten()
    }
}

pub struct PythonCodeGenerator {
    pub pkg_path: Utf8PathBuf,
    pub testing_pkg_path: Utf8PathBuf,
}

impl PythonCodeGenerator {
    pub fn new(pkg_path: impl Into<Utf8PathBuf>, testing_pkg_path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            pkg_path: pkg_path.into(),
            testing_pkg_path: testing_pkg_path.into(),
        }
    }
}

impl CodeGenerator for PythonCodeGenerator {
    fn generate(
        &mut self,
        reporter: &Reporter,
        objects: &Objects,
        type_registry: &TypeRegistry,
    ) -> GeneratedFiles {
        let mut files_to_write = GeneratedFiles::default();

        for object_kind in ObjectKind::ALL {
            self.generate_folder(
                reporter,
                objects,
                type_registry,
                object_kind,
                &mut files_to_write,
            );
        }

        {
            // TODO(jleibs): Should we still be generating an equivalent to this?
            /*
            let archetype_names = objects
                .objects_of_kind(ObjectKind::Archetype)
                .iter()
                .map(|o| o.name.clone())
                .collect_vec();
            files_to_write.insert(
                self.pkg_path.join("__init__.py"),
                lib_source_code(&archetype_names),
            );
            */
        }

        files_to_write
    }
}

/// `ExtensionClass` represents an optional means of extending the generated python code
///
/// For any given type the extension will be looked for using the `_ext.py` suffix in the same
/// directory as the type and must have a name ending with `Ext`.
///
/// For example, if the generated class for `Color` is found in `color.py`, then the `ExtensionClass`
/// should be `ColorExt` and found in the `color_ext.py` file.
///
/// If the `ExtensionClass` is found it will be added as another parent-class of the base type.
/// Python supports multiple-inheritance and often refers to this as a "mixin" class.
struct ExtensionClass {
    /// Whether or not the `ObjectExt` was found
    found: bool,

    /// The name of the file where the `ObjectExt` is implemented
    file_name: String,

    /// The name of the module where `ObjectExt` is implemented
    module_name: String,

    /// The name of this `ObjectExt`
    name: String,

    /// The discovered overrides for field converters.
    ///
    /// The overrides must end in [`FIELD_CONVERTER_SUFFIX`] in order to be discovered.
    ///
    /// If an extension class has a method named after the field with this suffix, it will be passed
    /// as the converter argument to the `attrs` field constructor.
    ///
    /// For example, `ColorExt` has a method `rgba__field_converter_override`. This results in
    /// the rgba field being created as:
    /// ```python
    /// rgba: int = field(converter=ColorExt.rgba__field_converter_override)
    /// ```
    field_converter_overrides: Vec<String>,

    /// Whether the `ObjectExt` contains __init__()
    ///
    /// If the `ExtensioNClass` contains its own `__init__`, we need to avoid creating the
    /// default `__init__` via `attrs.define`. This can be done by specifying:
    /// ```python
    /// @define(init=false)
    /// ```
    has_init: bool,

    /// Whether the `ObjectExt` contains __array__()
    ///
    /// If the `ExtensionClass` contains its own `__array__` then we avoid generating
    /// a default implementation.
    has_array: bool,

    /// Whether the `ObjectExt` contains `__native_to_pa_array__()`
    has_native_to_pa_array: bool,

    /// Whether the `ObjectExt` contains a `deferred_patch_class()` method
    has_deferred_patch_class: bool,

    /// Whether the `ObjectExt` contains __len__()
    ///
    /// If the `ExtensionClass` contains its own `__len__` then we avoid generating
    /// a default implementation.
    has_len: bool,
}

impl ExtensionClass {
    fn new(reporter: &Reporter, base_path: &Utf8Path, obj: &Object, objects: &Objects) -> Self {
        let file_name = format!("{}_ext.py", obj.snake_case_name());
        let ext_filepath = base_path.join(file_name.clone());
        let module_name = ext_filepath.file_stem().unwrap().to_owned();
        let mut name = obj.name.clone();
        name.push_str("Ext");

        if ext_filepath.exists() {
            let contents = std::fs::read_to_string(&ext_filepath)
                .with_context(|| format!("couldn't load overrides module at {ext_filepath:?}"))
                .unwrap();

            let scope = if let Some(scope) = obj.scope() {
                format!("{scope}.")
            } else {
                String::new()
            };

            let mandatory_docstring = format!(
                r#""""Extension for [{name}][rerun.{scope}{kind}.{name}].""""#,
                name = obj.name,
                kind = obj.kind.plural_snake_case()
            );
            if !contents.contains(&mandatory_docstring) {
                reporter.error(
                    ext_filepath.as_str(),
                    &obj.fqname,
                    format!(
                        "The following docstring should be added to the `class`: {mandatory_docstring}"
                    ),
                );
            }

            // Extract all methods
            // TODO(jleibs): Maybe pull in regex_light here
            let methods: Vec<_> = contents
                .lines()
                .map(|l| l.trim())
                .filter(|l| l.starts_with("def"))
                .map(|l| l.trim_start_matches("def").trim())
                .filter_map(|l| l.split('(').next())
                .collect();

            let has_init = methods.contains(&INIT_METHOD);

            // Verify that the __init__ method calls __attrs_init__ with all fields
            if has_init {
                check_ext_consistency(reporter, obj, objects, &contents, &ext_filepath);
            }
            let has_array = methods.contains(&ARRAY_METHOD);
            let has_len = methods.contains(&LEN_METHOD);
            let has_native_to_pa_array = methods.contains(&NATIVE_TO_PA_ARRAY_METHOD);
            let has_deferred_patch_class = methods.contains(&DEFERRED_PATCH_CLASS_METHOD);
            let field_converter_overrides: Vec<String> = methods
                .into_iter()
                .filter(|l| l.ends_with(FIELD_CONVERTER_SUFFIX))
                .map(|l| l.to_owned())
                .collect();

            let valid_converter_overrides = if obj.is_union() {
                itertools::Either::Left(iter::once("inner"))
            } else {
                itertools::Either::Right(obj.fields.iter().map(|field| field.name.as_str()))
            }
            .map(|field| format!("{field}{FIELD_CONVERTER_SUFFIX}"))
            .collect::<HashSet<_>>();

            for converter in &field_converter_overrides {
                if !valid_converter_overrides.contains(converter) {
                    reporter.error(
                        ext_filepath.as_str(),
                        &obj.fqname,
                        format!(
                            "The field converter override `{converter}` is not a valid field name.",
                        ),
                    );
                }
            }

            Self {
                found: true,
                file_name,
                module_name,
                name,
                field_converter_overrides,
                has_init,
                has_array,
                has_native_to_pa_array,
                has_deferred_patch_class,
                has_len,
            }
        } else {
            Self {
                found: false,
                file_name,
                module_name,
                name,
                field_converter_overrides: vec![],
                has_init: false,
                has_array: false,
                has_native_to_pa_array: false,
                has_deferred_patch_class: false,
                has_len: false,
            }
        }
    }
}

fn check_ext_consistency(
    reporter: &Reporter,
    obj: &Object,
    objects: &Objects,
    contents: &str,
    ext_filepath: &Utf8PathBuf,
) {
    // Collect expected field names - either direct fields or fields from referenced objects
    let mut expected_fields = HashSet::new();

    for field in &obj.fields {
        if obj.kind == ObjectKind::Archetype || obj.kind == ObjectKind::Datatype {
            // For archetypes/datatypes, always use the direct field names since they reference components
            // and we want to use the component names directly, not look inside the components
            expected_fields.insert(&field.name);
        } else {
            // For components and datatypes, check if this field references another rerun datatype
            if let Type::Object { fqname } = &field.typ {
                if let Some(referenced_obj) = objects.get(fqname) {
                    // Only apply field indirection if referencing another datatype, not component
                    if referenced_obj.kind == ObjectKind::Datatype {
                        // Use the referenced datatype's fields instead of the direct field name
                        for referenced_field in &referenced_obj.fields {
                            expected_fields.insert(&referenced_field.name);
                        }
                    } else {
                        // If referencing a component, use the direct field name
                        expected_fields.insert(&field.name);
                    }
                } else {
                    // Fallback to the direct field name if we can't find the referenced object
                    expected_fields.insert(&field.name);
                }
            } else {
                // For non-object types, use the direct field name
                expected_fields.insert(&field.name);
            }
        }
    }

    // Look for __attrs_init__ call using Python indentation structure
    if contents.contains("__attrs_init__") {
        let lines: Vec<&str> = contents.lines().collect();
        let mut attrs_init_section = String::new();
        let mut found_attrs_init = false;
        let mut base_indent = 0;

        for line in lines {
            if line.contains("__attrs_init__(") && !found_attrs_init {
                found_attrs_init = true;
                // Calculate the indentation of the __attrs_init__ line
                base_indent = line.len() - line.trim_start().len();
                attrs_init_section.push_str(line);
                attrs_init_section.push('\n');

                // Check if it's a single-line call (ends with ')' on the same line)
                if line.trim_end().ends_with(')') {
                    break;
                }
            } else if found_attrs_init {
                attrs_init_section.push_str(line);
                attrs_init_section.push('\n');

                // Check if this line has a ')' at the same or lesser indentation level
                let line_indent = line.len() - line.trim_start().len();
                if line.trim_start().starts_with(')') && line_indent <= base_indent {
                    break;
                }
            }
        }

        if found_attrs_init {
            // Extract field names using regex to find field_name=… patterns
            let mut found_fields = HashSet::new();
            for field_name in &expected_fields {
                let field_pattern = format!(r"\b{}\s*=", regex_lite::escape(field_name));
                let field_regex = Regex::new(&field_pattern).unwrap();
                if field_regex.is_match(&attrs_init_section) {
                    found_fields.insert(field_name);
                }
            }

            // Check if all expected fields are present
            for field_name in &expected_fields {
                if !found_fields.contains(field_name) {
                    reporter.error(
                        ext_filepath.as_str(),
                        &obj.fqname,
                        format!(
                            "The __init__ method should call __attrs_init__ with field '{field_name}={field_name}' parameter",
                        ),
                    );
                }
            }
        } else {
            reporter.error(
                ext_filepath.as_str(),
                &obj.fqname,
                "Could not find __attrs_init__ call".to_owned(),
            );
        }
    } else {
        reporter.error(
            ext_filepath.as_str(),
            &obj.fqname,
            "The __init__ method should call __attrs_init__ with all available fields".to_owned(),
        );
    }
}

struct ExtensionClasses {
    classes: BTreeMap<String, ExtensionClass>,
}

impl Deref for ExtensionClasses {
    type Target = BTreeMap<String, ExtensionClass>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.classes
    }
}

impl PythonCodeGenerator {
    fn generate_folder(
        &self,
        reporter: &Reporter,
        objects: &Objects,
        type_registry: &TypeRegistry,
        object_kind: ObjectKind,
        files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
    ) {
        let kind_path = self.pkg_path.join(object_kind.plural_snake_case());
        let test_kind_path = self.testing_pkg_path.join(object_kind.plural_snake_case());

        // (module_name, [object_name])
        let mut mods = BTreeMap::<String, Vec<String>>::new();
        let mut scoped_mods = BTreeMap::<String, BTreeMap<String, Vec<String>>>::new();
        let mut test_mods = BTreeMap::<String, Vec<String>>::new();

        let ext_classes = ExtensionClasses {
            classes: objects
                .objects_of_kind(object_kind)
                .map(|obj| {
                    let kind_path = if let Some(scope) = obj.scope() {
                        self.pkg_path
                            .join(scope)
                            .join(object_kind.plural_snake_case())
                    } else {
                        kind_path.clone()
                    };

                    let ext_class = ExtensionClass::new(reporter, &kind_path, obj, objects);

                    (obj.fqname.clone(), ext_class)
                })
                .collect(),
        };

        // Generate folder contents:
        for obj in objects.objects_of_kind(object_kind) {
            let scope = obj.scope();

            let kind_path = if let Some(scope) = scope {
                self.pkg_path
                    .join(scope)
                    .join(object_kind.plural_snake_case())
            } else {
                kind_path.clone()
            };

            let filepath = if obj.is_testing() {
                test_kind_path.join(format!("{}.py", obj.snake_case_name()))
            } else {
                kind_path.join(format!("{}.py", obj.snake_case_name()))
            };

            let ext_class = ext_classes
                .get(&obj.fqname)
                .expect("We created this for every object");

            let names = match obj.kind {
                ObjectKind::Datatype | ObjectKind::Component => {
                    let name = &obj.name;

                    if obj.is_delegating_component() {
                        vec![name.clone(), format!("{name}Batch")]
                    } else {
                        vec![
                            format!("{name}"),
                            format!("{name}ArrayLike"),
                            format!("{name}Batch"),
                            format!("{name}Like"),
                        ]
                    }
                }
                ObjectKind::View | ObjectKind::Archetype => vec![obj.name.clone()],
            };

            // NOTE: Isolating the file stem only works because we're handling datatypes, components
            // and archetypes separately (and even then it's a bit shady, eh).
            if obj.is_testing() {
                &mut test_mods
            } else if let Some(scope) = obj.scope() {
                scoped_mods.entry(scope).or_default()
            } else {
                &mut mods
            }
            .entry(filepath.file_stem().unwrap().to_owned())
            .or_default()
            .extend(names.iter().cloned());

            let mut code = String::new();
            code.push_indented(0, format!("# {}", autogen_warning!()), 1);
            if let Some(source_path) = obj.relative_filepath() {
                code.push_indented(0, format!("# Based on {:?}.", format_path(source_path)), 2);

                if obj.kind != ObjectKind::View {
                    // View type extension isn't implemented yet (shouldn't be hard though to add if needed).
                    code.push_indented(
                        0,
                        format!(
                            "# You can extend this class by creating a {:?} class in {:?}.",
                            ext_class.name, ext_class.file_name
                        ),
                        2,
                    );
                }
            }

            let manifest = quote_manifest(names);

            let rerun_path = if obj.is_testing() {
                "rerun."
            } else if obj.scope().is_some() {
                "..." // NOLINT
            } else {
                ".."
            };

            code.push_unindented(
                format!(
                    "
            from __future__ import annotations

            from collections.abc import Iterable, Mapping, Set, Sequence, Dict
            from typing import Any, Optional, Union, TYPE_CHECKING, SupportsFloat, Literal, Tuple
            from typing_extensions import deprecated # type: ignore[misc, unused-ignore]

            from attrs import define, field
            import numpy as np
            import numpy.typing as npt
            import pyarrow as pa
            import uuid

            from {rerun_path}_numpy_compatibility import asarray
            from {rerun_path}error_utils import catch_and_log_exceptions
            from {rerun_path}_baseclasses import (
                Archetype,
                BaseBatch,
                ComponentBatchMixin,
                ComponentColumn,
                ComponentColumnList,
                ComponentDescriptor,
                ComponentMixin,
                DescribedComponentBatch,
            )
            from {rerun_path}_converters import (
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
            "
                ),
                0,
            );

            if ext_class.found {
                code.push_unindented(
                    format!("from .{} import {}", ext_class.module_name, ext_class.name,),
                    1,
                );
            }

            if obj
                .try_get_attr::<String>(crate::ATTR_RERUN_VISUALIZER)
                .is_some()
            {
                code.push_unindented(
                    format!("from {rerun_path}blueprint import VisualizableArchetype, Visualizer"),
                    1,
                );
                code.push_unindented(
                    format!(
                        "from {rerun_path}blueprint.datatypes import VisualizerComponentMappingLike"
                    ),
                    1,
                );
            }

            let import_clauses: HashSet<_> = obj
                .fields
                .iter()
                .filter_map(|field| quote_import_clauses_from_field(&obj.scope(), field))
                .chain(obj.fields.iter().filter_map(|field| {
                    let fqname = field.typ.fqname()?;
                    objects[fqname].delegate_datatype(objects).map(|delegate| {
                        quote_import_clauses_from_fqname(&obj.scope(), &delegate.fqname)
                    })
                }))
                .collect();
            for clause in import_clauses {
                code.push_indented(0, &clause, 1);
            }

            if !manifest.is_empty() {
                code.push_unindented(format!("\n__all__ = [{manifest}]\n\n\n"), 0);
            }

            let obj_code = match obj.class {
                crate::objects::ObjectClass::Struct => {
                    if obj.kind == ObjectKind::View {
                        code_for_view(reporter, objects, ext_class, obj)
                    } else {
                        code_for_struct(
                            reporter,
                            type_registry,
                            ext_class,
                            objects,
                            &ext_classes,
                            obj,
                        )
                    }
                }
                crate::objects::ObjectClass::Enum(_) => {
                    code_for_enum(reporter, type_registry, ext_class, objects, obj)
                }
                crate::objects::ObjectClass::Union => code_for_union(
                    reporter,
                    type_registry,
                    ext_class,
                    objects,
                    &ext_classes,
                    obj,
                ),
            };

            code.push_indented(0, &obj_code, 1);

            if ext_class.has_deferred_patch_class {
                code.push_unindented(
                    format!("{}.deferred_patch_class({})", ext_class.name, obj.name),
                    1,
                );
            }

            files_to_write.insert(filepath.clone(), code);
        }

        // rerun/[{scope}]/{datatypes|components|archetypes|views}/__init__.py
        write_init_file(&kind_path, &mods, files_to_write);
        write_init_file(&test_kind_path, &test_mods, files_to_write);
        for (scope, mods) in scoped_mods {
            let scoped_kind_path = self
                .pkg_path
                .join(scope)
                .join(object_kind.plural_snake_case());
            write_init_file(&scoped_kind_path, &mods, files_to_write);
        }
    }
}

fn write_init_file(
    kind_path: &Utf8PathBuf,
    mods: &BTreeMap<String, Vec<String>>,
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    if mods.is_empty() {
        return;
    }

    let path = kind_path.join("__init__.py");
    let mut code = String::new();
    let manifest = quote_manifest(mods.iter().flat_map(|(_, names)| names.iter()));
    code.push_indented(0, format!("# {}", autogen_warning!()), 2);
    code.push_unindented(
        "
            from __future__ import annotations

            ",
        0,
    );
    for (module, names) in mods {
        let names = names.join(", ");
        code.push_indented(0, format!("from .{module} import {names}"), 1);
    }
    if !manifest.is_empty() {
        code.push_unindented(format!("\n__all__ = [{manifest}]"), 0);
    }
    files_to_write.insert(path, code);
}

#[expect(dead_code)]
fn lib_source_code(archetype_names: &[String]) -> String {
    let manifest = quote_manifest(archetype_names);
    let archetype_names = archetype_names.join(", ");

    let mut code = String::new();

    code += &unindent(&format!(
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
    reporter: &Reporter,
    type_registry: &TypeRegistry,
    ext_class: &ExtensionClass,
    objects: &Objects,
    ext_classes: &ExtensionClasses,
    obj: &Object,
) -> String {
    assert!(obj.is_struct());

    let Object {
        name, kind, fields, ..
    } = obj;

    let mut code = String::new();

    // field converters preprocessing pass — must be performed here because we must autogen
    // converter function *before* the class
    let mut field_converters: HashMap<String, String> = HashMap::new();

    if !obj.is_delegating_component() {
        for field in fields {
            let (default_converter, converter_function) =
                quote_field_converter_from_field(obj, objects, ext_classes, field);

            let converter_override_name = format!("{}{FIELD_CONVERTER_SUFFIX}", field.name);

            let converter = if ext_class
                .field_converter_overrides
                .contains(&converter_override_name)
            {
                format!("converter={}.{converter_override_name}", ext_class.name)
            } else if *kind == ObjectKind::Archetype {
                // Archetypes use the ComponentBatch constructor for their fields
                let (typ_unwrapped, _) = quote_field_type_from_field(objects, field, true);
                format!("converter={typ_unwrapped}Batch._converter,  # type: ignore[misc]\n")
            } else if !default_converter.is_empty() {
                code.push_indented(0, &converter_function, 1);
                format!("converter={default_converter}")
            } else {
                String::new()
            };
            field_converters.insert(field.fqname.clone(), converter);
        }
    }

    let mut superclasses = vec![];

    // Extension class needs to come first, so its __init__ method is called if there is one.
    if ext_class.found {
        superclasses.push(ext_class.name.clone());
    }

    if *kind == ObjectKind::Archetype {
        superclasses.push("Archetype".to_owned());
    }

    let visualizer_name = obj.try_get_attr::<String>(crate::ATTR_RERUN_VISUALIZER);
    if visualizer_name.is_some() {
        superclasses.push("VisualizableArchetype".to_owned());
    }

    // Delegating component inheritance comes after the `ExtensionClass`
    // This way if a component needs to override `__init__` it still can.
    if obj.is_delegating_component() {
        let delegate = obj.delegate_datatype(objects).unwrap();
        let scope = match delegate.scope() {
            Some(scope) => format!("{scope}_"),
            None => String::new(),
        };
        superclasses.push(format!(
            "{scope}datatypes.{}",
            obj.delegate_datatype(objects).unwrap().name
        ));
    }

    if *kind == ObjectKind::Component {
        superclasses.push("ComponentMixin".to_owned());
    }

    if let Some(deprecation_summary) = obj.deprecation_summary() {
        code.push_unindented(format!(r#"@deprecated("""{deprecation_summary}""")"#), 1);
    }

    if !obj.is_delegating_component() {
        let define_args = if *kind == ObjectKind::Archetype {
            "str=False, repr=False, init=False"
        } else {
            "init=False"
        };
        code.push_unindented(format!("@define({define_args})"), 1);
    }

    let superclass_decl = if superclasses.is_empty() {
        String::new()
    } else {
        format!("({})", superclasses.join(","))
    };
    code.push_unindented(format!("class {name}{superclass_decl}:"), 1);

    code.push_indented(1, quote_obj_docs(reporter, objects, obj), 0);

    if *kind == ObjectKind::Component {
        code.push_indented(1, "_BATCH_TYPE = None", 1);
    }

    if ext_class.has_init {
        code.push_indented(
            1,
            format!("# __init__ can be found in {}", ext_class.file_name),
            2,
        );
    } else if obj.is_delegating_component() {
        code.push_indented(
            1,
            format!(
                "# You can define your own __init__ function as a member of {} in {}",
                ext_class.name, ext_class.file_name
            ),
            2,
        );
    } else {
        // In absence of a an extension class __init__ method, we don't *need* an __init__ method here.
        // But if we don't generate one, LSP will show the class's doc string instead of parameter documentation.
        code.push_indented(1, quote_init_method(reporter, obj, ext_class, objects), 2);
    }

    // Generate __bool__ operator if this is a single field struct with a bool field.
    if fields.len() == 1 && fields[0].typ == Type::Bool {
        code.push_indented(
            1,
            format!(
                "def __bool__(self) -> bool:
    return self.{}
",
                fields[0].name
            ),
            2,
        );
    }

    if obj.kind == ObjectKind::Archetype {
        code.push_indented(1, quote_clear_methods(obj), 2);
        code.push_indented(1, quote_partial_update_methods(reporter, obj, objects), 2);
        if obj.scope().is_none() {
            code.push_indented(1, quote_columnar_methods(reporter, obj, objects), 2);
        }
    }

    if obj.is_delegating_component() {
        code.push_indented(
            1,
            format!(
                "# Note: there are no fields here because {} delegates to datatypes.{}",
                obj.name,
                obj.delegate_datatype(objects).unwrap().name
            ),
            1,
        );

        code.push_indented(1, "pass", 2);
    } else {
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
                name, is_nullable, ..
            } = field;

            let (typ, _) = quote_field_type_from_field(objects, field, false);
            let (typ_unwrapped, _) = quote_field_type_from_field(objects, field, true);
            let typ = if *kind == ObjectKind::Archetype {
                format!("{typ_unwrapped}Batch")
            } else {
                typ
            };

            let metadata = if *kind == ObjectKind::Archetype {
                "\nmetadata={'component': True}, ".to_owned()
            } else {
                String::new()
            };

            let converter = &field_converters[&field.fqname];
            let type_ignore = if converter.contains("Ext.") {
                // Leading commas is important here to force predictable wrapping
                // or else the ignore ends up on the wrong line.
                ", # type: ignore[misc]".to_owned()
            } else {
                String::new()
            };
            // Note: mypy gets confused using staticmethods for field-converters
            let typ = if !obj.is_archetype() && !*is_nullable {
                format!("{typ} = field(\n{metadata}{converter}{type_ignore}\n)")
            } else {
                format!(
                    "{typ} | None = field(\n{metadata}default=None{}{converter}{type_ignore}\n)",
                    if converter.is_empty() { "" } else { ", " },
                )
            };

            code.push_indented(1, format!("{name}: {typ}"), 1);

            // Generating docs for all the fields creates A LOT of visual noise in the API docs.
            let show_fields_in_docs = false;
            let doc_lines = lines_from_docs(reporter, objects, &field.docs, &field.state);
            if !doc_lines.is_empty() {
                if show_fields_in_docs {
                    code.push_indented(1, quote_doc_lines(doc_lines), 0);
                } else {
                    // Still include it for those that are reading the source file:
                    for line in doc_lines {
                        code.push_indented(1, format!("# {line}"), 1);
                    }
                    code.push_indented(1, "#", 1);
                    code.push_indented(1, "# (Docstring intentionally commented out to hide this field from the docs)", 2);
                }
            }
        }

        if *kind == ObjectKind::Archetype {
            code.push_indented(1, "__str__ = Archetype.__str__", 1);
            code.push_indented(
                1,
                "__repr__ = Archetype.__repr__ # type: ignore[assignment] ",
                1,
            );

            if let Some(visualizer_name) = visualizer_name {
                // TODO(#10631): Marked as experimental
                let docstring = r#""""
        Creates a visualizer for this archetype, using all currently set values as overrides.

        Parameters
        ----------
        mappings:
            Optional component mappings to control how the visualizer sources its data.

            ⚠️ **Experimental**: Component mappings are an experimental feature and may change.
            See https://github.com/rerun-io/rerun/issues/10631 for more information.

        """"#;
                code.push_indented(1, "", 1);
                code.push_indented(1, "def visualizer(self, *, mappings: list[VisualizerComponentMappingLike] | None = None) -> Visualizer:", 1);
                code.push_indented(2, docstring, 1);
                code.push_indented(2, format!(r#"return Visualizer("{visualizer_name}", overrides=self.as_component_batches(), mappings=mappings)"#), 1);
            }
        }

        code.push_indented(1, quote_array_method_from_obj(ext_class, objects, obj), 1);
        code.push_indented(1, quote_native_types_method_from_obj(objects, obj), 1);
        code.push_indented(1, quote_len_method_from_obj(ext_class, obj), 1);

        if *kind != ObjectKind::Archetype {
            code.push_indented(0, quote_aliases_from_object(obj), 1);
        }
    }

    match kind {
        ObjectKind::Archetype => (),
        ObjectKind::Component => {
            code.push_indented(
                0,
                quote_arrow_support_from_obj(reporter, type_registry, ext_class, objects, obj),
                1,
            );

            code.push_indented(
                0,
                format!(
                    "# This is patched in late to avoid circular dependencies.
{name}._BATCH_TYPE = {name}Batch  # type: ignore[assignment]"
                ),
                1,
            );
        }
        ObjectKind::Datatype => {
            code.push_indented(
                0,
                quote_arrow_support_from_obj(reporter, type_registry, ext_class, objects, obj),
                1,
            );
        }
        ObjectKind::View => {
            unreachable!("View processing shouldn't reach struct generation code.");
        }
    }

    code
}

fn code_for_enum(
    reporter: &Reporter,
    type_registry: &TypeRegistry,
    ext_class: &ExtensionClass,
    objects: &Objects,
    obj: &Object,
) -> String {
    assert!(obj.class.is_enum());
    assert!(matches!(
        obj.kind,
        ObjectKind::Datatype | ObjectKind::Component
    ));

    let Object {
        name: enum_name, ..
    } = obj;

    let mut code = String::new();

    code.push_unindented("from enum import Enum", 2);

    if let Some(deprecation_summary) = obj.deprecation_summary() {
        code.push_unindented(format!(r#"@deprecated("""{deprecation_summary}""")"#), 1);
    }
    let superclasses = {
        let mut superclasses = vec![];
        if ext_class.found {
            // Extension class needs to come first, so its __init__ method is called if there is one.
            superclasses.push(ext_class.name.clone());
        }
        superclasses.push("Enum".to_owned());
        superclasses.join(",")
    };
    code.push_str(&format!("class {enum_name}({superclasses}):\n"));
    code.push_indented(1, quote_obj_docs(reporter, objects, obj), 0);

    for variant in &obj.fields {
        let enum_value = obj
            .enum_integer_type()
            .expect("enums must have an integer type")
            .format_value(
                variant
                    .enum_or_union_variant_value
                    .expect("enums fields must have values"),
            );

        // NOTE: we keep the casing of the enum variants exactly as specified in the .fbs file,
        // or else `RGBA` would become `Rgba` and so on.
        // Note that we want consistency across:
        // * all languages (C++, Python, Rust)
        // * the arrow datatype
        // * the GUI
        let variant_name = &variant.name;
        code.push_indented(1, format!("{variant_name} = {enum_value}"), 1);

        // Generating docs for all the fields creates A LOT of visual noise in the API docs.
        let show_fields_in_docs = true;
        let doc_lines = lines_from_docs(reporter, objects, &variant.docs, &variant.state);
        if !doc_lines.is_empty() {
            if show_fields_in_docs {
                code.push_indented(1, quote_doc_lines(doc_lines), 0);
            } else {
                // Still include it for those that are reading the source file:
                for line in doc_lines {
                    code.push_indented(1, format!("# {line}"), 1);
                }
                code.push_indented(1, "#", 1);
                code.push_indented(
                    1,
                    "# (Docstring intentionally commented out to hide this field from the docs)",
                    2,
                );
            }
        }
    }

    // -------------------------------------------------------

    // Flexible constructor:
    code.push_indented(
        1,
        format!(
            r#"
@classmethod
{extra_decorators}
def auto(cls, val: str | int | {enum_name}) -> {enum_name}:
    '''Best-effort converter, including a case-insensitive string matcher.'''
    if isinstance(val, {enum_name}):
        return val
    if isinstance(val, int):
        return cls(val)
    try:
        return cls[val]
    except KeyError:
        val_lower = val.lower()
        for variant in cls:
            if variant.name.lower() == val_lower:
                return variant
    raise ValueError(f"Cannot convert {{val}} to {{cls.__name__}}")
        "#,
            extra_decorators = classmethod_decorators(obj)
        ),
        1,
    );

    // Overload `__str__`:
    code.push_indented(1, "def __str__(self) -> str:", 1);
    code.push_indented(2, "'''Returns the variant name.'''", 1);

    code.push_indented(2, "return self.name", 1);

    // -------------------------------------------------------

    // -------------------------------------------------------

    let variants = format!(
        "Literal[{}]",
        itertools::chain!(
            // We always accept the original casing
            obj.fields.iter().map(|v| format!("{:?}", v.name)),
            // We also accept the lowercase variant, for historical reasons (and maybe others?)
            obj.fields
                .iter()
                .map(|v| format!("{:?}", v.name.to_lowercase()))
        )
        .sorted()
        .dedup()
        .join(", ")
    );
    code.push_unindented(
        format!("{enum_name}Like = {enum_name} | {variants} | int"),
        1,
    );
    code.push_unindented(
        format!(
            r#"
            """A type alias for any {enum_name}-like object."""
            "#,
        ),
        1,
    );
    code.push_unindented(
        format!(
            r#"
            {enum_name}ArrayLike = {enum_name} | {variants} | int |Sequence[{enum_name}Like]
            """A type alias for any {enum_name}-like array object."""
            "#,
        ),
        2,
    );

    match obj.kind {
        ObjectKind::Archetype => {
            reporter.error(&obj.virtpath, &obj.fqname, "An archetype cannot be an enum");
        }
        ObjectKind::Component | ObjectKind::Datatype => {
            code.push_indented(
                0,
                quote_arrow_support_from_obj(reporter, type_registry, ext_class, objects, obj),
                1,
            );
        }
        ObjectKind::View => {
            reporter.error(&obj.virtpath, &obj.fqname, "A view cannot be an enum");
        }
    }

    code
}

fn code_for_union(
    reporter: &Reporter,
    type_registry: &TypeRegistry,
    ext_class: &ExtensionClass,
    objects: &Objects,
    ext_classes: &ExtensionClasses,
    obj: &Object,
) -> String {
    assert_eq!(obj.class, ObjectClass::Union);
    assert_eq!(obj.kind, ObjectKind::Datatype);

    let Object {
        name, kind, fields, ..
    } = obj;

    let mut code = String::new();

    // init override handling
    let define_args = if ext_class.has_init {
        "(init=False)".to_owned()
    } else {
        String::new()
    };

    let superclass_decl = {
        let mut superclasses = vec![];

        // Extension class needs to come first, so its __init__ method is called if there is one.
        if ext_class.found {
            superclasses.push(ext_class.name.as_str());
        }

        if *kind == ObjectKind::Archetype {
            superclasses.push("Archetype");
        }

        if superclasses.is_empty() {
            String::new()
        } else {
            format!("({})", superclasses.join(","))
        }
    };

    if let Some(deprecation_summary) = obj.deprecation_summary() {
        code.push_unindented(format!(r#"@deprecated("""{deprecation_summary}""")"#), 1);
    }

    code.push_unindented(
        format!(
            r#"

                @define{define_args}
                class {name}{superclass_decl}:
                "#
        ),
        0,
    );

    code.push_indented(1, quote_obj_docs(reporter, objects, obj), 0);

    if ext_class.has_init {
        code.push_indented(
            1,
            format!("# __init__ can be found in {}", ext_class.file_name),
            2,
        );
    } else {
        code.push_indented(
            1,
            format!(
                "# You can define your own __init__ function as a member of {} in {}",
                ext_class.name, ext_class.file_name
            ),
            2,
        );
    }

    let field_types = fields
        .iter()
        .map(|f| quote_field_type_from_field(objects, f, false).0)
        .collect::<BTreeSet<_>>();
    let has_duplicate_types = field_types.len() != fields.len();

    // provide a default converter if *all* arms are of the same type
    let default_converter = if field_types.len() == 1 {
        quote_field_converter_from_field(obj, objects, ext_classes, &fields[0]).0
    } else {
        String::new()
    };

    let inner_type = if field_types.len() > 1 {
        field_types.iter().join(" | ")
    } else {
        field_types.iter().next().unwrap().clone()
    };

    // components and datatypes have converters only if manually provided
    let converter_override_name = format!("inner{FIELD_CONVERTER_SUFFIX}");

    let converter = if ext_class
        .field_converter_overrides
        .contains(&converter_override_name)
    {
        format!("converter={}.{converter_override_name}", ext_class.name)
    } else if !default_converter.is_empty() {
        format!("converter={default_converter}")
    } else {
        String::new()
    };

    let type_ignore = if converter.contains("Ext.") {
        "# type: ignore[misc]".to_owned()
    } else {
        String::new()
    };

    // Note: mypy gets confused using staticmethods for field-converters
    code.push_indented(
        1,
        format!("inner: {inner_type} = field({converter} {type_ignore}\n)"),
        1,
    );
    code.push_indented(1, quote_doc_from_fields(reporter, objects, fields), 0);

    // if there are duplicate types, we need to add a `kind` field to disambiguate the union
    if has_duplicate_types {
        let kind_type = fields
            .iter()
            .map(|f| format!("{:?}", f.snake_case_name()))
            .join(", ");
        let first_kind = &fields[0].snake_case_name();

        code.push_indented(
            1,
            format!("kind: Literal[{kind_type}] = field(default={first_kind:?})"),
            1,
        );

        code.push_indented(
            1,
            quote_union_kind_from_fields(reporter, objects, fields),
            0,
        );
    }

    code.push_unindented(quote_union_aliases_from_object(obj, field_types.iter()), 1);

    match kind {
        ObjectKind::Archetype => (),
        ObjectKind::Component => {
            reporter.error(&obj.virtpath, &obj.fqname, "An component cannot be an enum");
        }
        ObjectKind::Datatype => {
            code.push_indented(
                0,
                quote_arrow_support_from_obj(reporter, type_registry, ext_class, objects, obj),
                1,
            );
        }
        ObjectKind::View => {
            reporter.error(&obj.virtpath, &obj.fqname, "An view cannot be an enum");
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

fn quote_examples(examples: Vec<Example<'_>>, lines: &mut Vec<String>) {
    let mut examples = examples.into_iter().peekable();
    while let Some(example) = examples.next() {
        let ExampleInfo {
            path,
            name,
            title,
            image,
            ..
        } = &example.base;

        for line in &example.lines {
            assert!(
                !line.contains("```"),
                "Example {path:?} contains ``` in it, so we can't embed it in the Python API docs."
            );
            assert!(
                !line.contains("\"\"\""),
                "Example {path:?} contains \"\"\" in it, so we can't embed it in the Python API docs."
            );
        }

        if let Some(title) = title {
            lines.push(format!("### {title}:"));
        } else {
            lines.push(format!("### `{name}`:"));
        }
        lines.push("```python".into());
        lines.extend(example.lines.into_iter());
        lines.push("```".into());
        if let Some(image) = &image {
            lines.extend(
                // Don't let the images take up too much space on the page.
                image.image_stack().center().width(640).finish(),
            );
        }
        if examples.peek().is_some() {
            // blank line between examples
            lines.push(String::new());
        }
    }
}

/// Ends with double newlines, unless empty.
fn quote_obj_docs(reporter: &Reporter, objects: &Objects, obj: &Object) -> String {
    let mut lines = lines_from_docs(reporter, objects, &obj.docs, &obj.state);

    if let Some(first_line) = lines.first_mut() {
        // Prefix with object kind:
        *first_line = format!("**{}**: {}", obj.kind.singular_name(), first_line);
    }

    quote_doc_lines(lines)
}

fn lines_from_docs(
    reporter: &Reporter,
    objects: &Objects,
    docs: &Docs,
    state: &State,
) -> Vec<String> {
    let mut lines = docs.lines_for(reporter, objects, Target::Python);

    if let Some(docline_summary) = state.docline_summary() {
        lines.push(String::new());
        lines.push(docline_summary);
    }

    let examples = collect_snippets_for_api_docs(docs, "py", true).unwrap_or_else(|err| {
        reporter.error_any(err);
        vec![]
    });

    if !examples.is_empty() {
        lines.push(String::new());
        let (section_title, divider) = if examples.len() == 1 {
            ("Example", "-------")
        } else {
            ("Examples", "--------")
        };
        lines.push(section_title.into());
        lines.push(divider.into());
        quote_examples(examples, &mut lines);
    }

    lines
}

/// Ends with double newlines, unless empty.
fn quote_doc_lines(lines: Vec<String>) -> String {
    if lines.is_empty() {
        return String::new();
    }

    for line in &lines {
        assert!(
            !line.contains("\"\"\""),
            "Cannot put triple quotes in Python docstrings"
        );
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise…
    let lines: Vec<String> = lines
        .into_iter()
        .filter(|line| !line.starts_with(r#"""""#))
        .collect();

    if lines.len() == 1 {
        // single-line
        let line = &lines[0];
        format!("\"\"\"{line}\"\"\"\n\n") // NOLINT
    } else {
        // multi-line
        format!("\"\"\"\n{}\n\"\"\"\n\n", lines.join("\n"))
    }
}

fn quote_doc_from_fields(
    reporter: &Reporter,
    objects: &Objects,
    fields: &Vec<ObjectField>,
) -> String {
    let mut lines = vec!["Must be one of:".to_owned(), String::new()];

    for field in fields {
        let mut content = field.docs.lines_for(reporter, objects, Target::Python);
        for line in &mut content {
            if line.starts_with(char::is_whitespace) {
                line.remove(0);
            }
        }

        let examples = collect_snippets_for_api_docs(&field.docs, "py", true).unwrap();
        if !examples.is_empty() {
            content.push(String::new()); // blank line between docs and examples
            quote_examples(examples, &mut lines);
        }
        lines.push(format!(
            "* {} ({}):",
            field.name,
            quote_field_type_from_field(objects, field, false).0
        ));
        lines.extend(content.into_iter().map(|line| format!("    {line}")));
        lines.push(String::new());
    }

    if lines.is_empty() {
        return String::new();
    } else {
        // remove last empty line
        lines.pop();
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise…
    let doc = lines
        .into_iter()
        .filter(|line| !line.starts_with(r#"""""#))
        .collect_vec()
        .join("\n");

    format!("\"\"\"\n{doc}\n\"\"\"\n\n")
}

fn quote_union_kind_from_fields(
    reporter: &Reporter,
    objects: &Objects,
    fields: &Vec<ObjectField>,
) -> String {
    let mut lines = vec!["Possible values:".to_owned(), String::new()];

    for field in fields {
        let mut content = field.docs.lines_for(reporter, objects, Target::Python);
        for line in &mut content {
            if line.starts_with(char::is_whitespace) {
                line.remove(0);
            }
        }
        lines.push(format!("* {:?}:", field.snake_case_name()));
        lines.extend(content.into_iter().map(|line| format!("    {line}")));
        lines.push(String::new());
    }

    if lines.is_empty() {
        return String::new();
    } else {
        // remove last empty line
        lines.pop();
    }

    // NOTE: Filter out docstrings within docstrings, it just gets crazy otherwise…
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
    ext_class: &ExtensionClass,
    objects: &Objects,
    obj: &Object,
) -> String {
    // TODO(cmc): should be using the native type, but need to compare numpy types etc
    let typ = quote_field_type_from_field(objects, &obj.fields[0], false).0;

    // allow overriding the __array__ function
    if ext_class.has_array {
        return format!("# __array__ can be found in {}", ext_class.file_name);
    }

    // exclude archetypes, objects which don't have a single field, and anything that isn't an numpy
    // array or scalar numbers
    if obj.kind == ObjectKind::Archetype
        || obj.fields.len() != 1
        || (!["npt.ArrayLike", "float", "int"].contains(&typ.as_str())
            && !typ.contains("npt.NDArray"))
    {
        return String::new();
    }

    let field_name = &obj.fields[0].name;
    unindent(&format!(
        "
        def __array__(self, dtype: npt.DTypeLike=None, copy: bool|None=None) -> npt.NDArray[Any]:
            # You can define your own __array__ function as a member of {} in {}
            return asarray(self.{field_name}, dtype=dtype, copy=copy)
        ",
        ext_class.name, ext_class.file_name
    ))
}

fn quote_len_method_from_obj(ext_class: &ExtensionClass, obj: &Object) -> String {
    // allow overriding the __len__ function
    if ext_class.has_len {
        return format!("# __len__ can be found in {}", ext_class.file_name);
    }

    // exclude archetypes, objects which don't have a single field, and anything that isn't plural
    if obj.kind == ObjectKind::Archetype || obj.fields.len() != 1 || !obj.fields[0].typ.is_plural()
    {
        return String::new();
    }

    let field_name = &obj.fields[0].name;

    let null_string = if obj.fields[0].is_nullable {
        // If the field is optional, we return 0 if it is None.
        format!(" if self.{field_name} is not None else 0")
    } else {
        String::new()
    };

    unindent(&format!(
        "
        def __len__(self) -> int:
            # You can define your own __len__ function as a member of {} in {}
            return len(self.{field_name}){null_string}
        ",
        ext_class.name, ext_class.file_name
    ))
}

/// Automatically implement `__str__`, `__int__`, or `__float__` as well as `__hash__` methods if the object has a single
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
    unindent(&format!(
        "
        def __{typ}__(self) -> {typ}:
            return {typ}(self.{field_name})

        def __hash__(self) -> int:
            return hash(self.{field_name})
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

fn quote_import_clauses_from_field(
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

fn quote_import_clauses_from_fqname(obj_scope: &Option<String>, fqname: &str) -> String {
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
fn quote_field_converter_from_field(
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

fn fqname_to_type(fqname: &str) -> String {
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

fn quote_type_from_type(typ: &Type) -> String {
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

fn quote_type_from_element_type(typ: &ElementType) -> String {
    quote_type_from_type(&Type::from(typ.clone()))
}

/// Arrow support objects
///
/// Generated for Components using native types and Datatypes. Components using a Datatype instead
/// delegate to the Datatype's arrow support.
fn quote_arrow_support_from_obj(
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

fn np_dtype_from_type(t: &Type) -> Option<&'static str> {
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
fn quote_arrow_serialization(
    reporter: &Reporter,
    objects: &Objects,
    obj: &Object,
    type_registry: &TypeRegistry,
    ext_class: &ExtensionClass,
) -> Result<String, String> {
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

fn quote_local_batch_type_imports(fields: &[ObjectField], current_obj_is_testing: bool) -> String {
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

fn quote_parameter_type_alias(
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

fn compute_init_parameters(obj: &Object, objects: &Objects) -> Vec<String> {
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

fn compute_init_parameter_docs(
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

fn quote_init_method(
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

fn quote_clear_methods(obj: &Object) -> String {
    let param_nones = obj
        .fields
        .iter()
        .map(|field| format!("{} = None,", field.name))
        .join("\n                ");

    let classname = &obj.name;

    unindent(&format!(
        r#"
        def __attrs_clear__(self) -> None:
            """Convenience method for calling `__attrs_init__` with all `None`s."""
            self.__attrs_init__(
                {param_nones}
            )

        @classmethod
        {extra_decorators}
        def _clear(cls) -> {classname}:
            """Produce an empty {classname}, bypassing `__init__`."""
            inst = cls.__new__(cls)
            inst.__attrs_clear__()
            return inst
        "#,
        extra_decorators = classmethod_decorators(obj)
    ))
}

fn quote_kwargs(obj: &Object) -> String {
    obj.fields
        .iter()
        .map(|field| {
            let field_name = field.snake_case_name();
            format!("'{field_name}': {field_name}")
        })
        .collect_vec()
        .join(",\n")
}

fn quote_component_field_mapping(obj: &Object) -> String {
    obj.fields
        .iter()
        .map(|field| {
            let field_name = field.snake_case_name();
            format!("'{}:{field_name}': {field_name}", obj.name)
        })
        .collect_vec()
        .join(",\n")
}

fn quote_partial_update_methods(reporter: &Reporter, obj: &Object, objects: &Objects) -> String {
    let name = &obj.name;

    let parameters = obj
        .fields
        .iter()
        .map(|field| {
            let mut field = field.clone();
            field.is_nullable = true;
            quote_init_parameter_from_field(&field, objects, &obj.fqname)
        })
        .collect_vec()
        .join(",\n");
    let parameters = indent::indent_by(8, parameters);

    let kwargs = quote_kwargs(obj);
    let kwargs = indent::indent_by(12, kwargs);

    let parameter_docs = compute_init_parameter_docs(reporter, obj, objects);
    let mut doc_string_lines = vec![format!("Update only some specific fields of a `{name}`.")];
    if !parameter_docs.is_empty() {
        doc_string_lines.push("\n".to_owned());
        doc_string_lines.push("Parameters".to_owned());
        doc_string_lines.push("----------".to_owned());
        doc_string_lines.push("clear_unset:".to_owned());
        doc_string_lines
            .push("    If true, all unspecified fields will be explicitly cleared.".to_owned());
        for doc in parameter_docs {
            doc_string_lines.push(doc);
        }
    }
    let doc_block = indent::indent_by(12, quote_doc_lines(doc_string_lines));

    unindent(&format!(
        r#"
        @classmethod
        {extra_decorators}
        def from_fields(
            cls,
            *,
            clear_unset: bool = False,
            {parameters},
        ) -> {name}:
            {doc_block}
            inst = cls.__new__(cls)
            with catch_and_log_exceptions(context=cls.__name__):
                kwargs = {{
                    {kwargs},
                }}

                if clear_unset:
                    kwargs = {{k: v if v is not None else [] for k, v in kwargs.items()}}  # type: ignore[misc]

                inst.__attrs_init__(**kwargs)
                return inst

            inst.__attrs_clear__()
            return inst

        @classmethod
        def cleared(cls) -> {name}:
            """Clear all the fields of a `{name}`."""
            return cls.from_fields(clear_unset=True)
        "#,
        extra_decorators = classmethod_decorators(obj)
    ))
}

fn quote_columnar_methods(reporter: &Reporter, obj: &Object, objects: &Objects) -> String {
    let parameters = obj
        .fields
        .iter()
        .filter_map(|field| {
            let mut field = field.make_plural()?;
            field.is_nullable = true;
            Some(quote_init_parameter_from_field(
                &field,
                objects,
                &obj.fqname,
            ))
        })
        .collect_vec()
        .join(",\n");
    let parameters = indent::indent_by(8, parameters);

    let init_args = obj
        .fields
        .iter()
        .map(|field| {
            let field_name = field.snake_case_name();
            format!("{field_name}={field_name}")
        })
        .collect_vec()
        .join(",\n");
    let init_args = indent::indent_by(12, init_args);

    let parameter_docs = compute_init_parameter_docs(reporter, obj, objects);
    let doc = unindent(
        "\
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.
        ",
    );
    let mut doc_string_lines = doc.lines().map(|s| s.to_owned()).collect_vec();
    if !parameter_docs.is_empty() {
        doc_string_lines.push("Parameters".to_owned());
        doc_string_lines.push("----------".to_owned());
        for doc in parameter_docs {
            doc_string_lines.push(doc);
        }
    }
    let doc_block = indent::indent_by(12, quote_doc_lines(doc_string_lines));

    let kwargs = quote_component_field_mapping(obj);
    let kwargs = indent::indent_by(12, kwargs);

    // NOTE: Calling `update_fields` is not an option: we need to be able to pass
    // plural data, even to singular fields (mono-components).
    unindent(&format!(
        r#"
        @classmethod
        {extra_decorators}
        def columns(
            cls,
            *,
            {parameters},
        ) -> ComponentColumnList:
            {doc_block}
            inst = cls.__new__(cls)
            with catch_and_log_exceptions(context=cls.__name__):
                inst.__attrs_init__(
                    {init_args},
                )

            batches = inst.as_component_batches()
            if len(batches) == 0:
                return ComponentColumnList([])

            kwargs = {{
                {kwargs}
            }}
            columns = []

            for batch in batches:
                arrow_array = batch.as_arrow_array()

                # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
                if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                    param = kwargs[batch.component_descriptor().component] # type: ignore[index]
                    shape = np.shape(param)  # type: ignore[arg-type]
                    elem_flat_len = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                    if pa.types.is_fixed_size_list(arrow_array.type) and arrow_array.type.list_size == elem_flat_len:
                        # If the product of the last dimensions of the shape are equal to the size of the fixed size list array,
                        # we have `num_rows` single element batches (each element is a fixed sized list).
                        # (This should have been already validated by conversion to the arrow_array)
                        batch_length = 1
                    else:
                        batch_length = shape[1] if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                    num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                    sizes = batch_length * np.ones(num_rows)
                else:
                    # For non-primitive types, default to partitioning each element separately.
                    sizes = np.ones(len(arrow_array))

                columns.append(batch.partition(sizes))

            return ComponentColumnList(columns)
        "#,
        extra_decorators = classmethod_decorators(obj)
    ))
}

// --- Arrow registry code generators ---

fn quote_arrow_datatype(datatype: &DataType) -> String {
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

fn quote_arrow_field(field: &Field) -> String {
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

fn quote_metadata_map(metadata: &BTreeMap<String, String>) -> String {
    let kvs = metadata
        .iter()
        .map(|(k, v)| format!("{k:?}, {v:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{kvs}}}")
}
