//! Implements the Python codegen pass.

mod archetype_methods;
mod arrow;
mod codegen;
mod docs;
mod extension_class;
mod init_method;
mod object_ext;
mod typing;
mod views;

use std::collections::{BTreeMap, HashSet};

use camino::Utf8PathBuf;
use unindent::unindent;

use self::codegen::{code_for_enum, code_for_struct, code_for_union};
use self::extension_class::{ExtensionClass, ExtensionClasses};
use self::object_ext::PythonObjectExt as _;
use self::typing::{quote_import_clauses_from_field, quote_import_clauses_from_fqname};
use self::views::code_for_view;
use crate::codegen::{StringExt as _, autogen_warning};
use crate::{CodeGenerator, GeneratedFiles, Object, ObjectKind, Objects, Reporter, TypeRegistry};

// ---

fn quote_manifest(names: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut quoted_names: Vec<_> = names
        .into_iter()
        .map(|name| format!("{:?}", name.as_ref()))
        .collect();
    quoted_names.sort();

    quoted_names.join(", ")
}

fn classmethod_decorators(obj: &Object) -> String {
    // We need to decorate all class methods as deprecated
    if let Some(deprecation_summary) = obj.deprecation_summary() {
        format!(r#"@deprecated("""{deprecation_summary}""")"#)
    } else {
        Default::default()
    }
}

// ---

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
                code.push_indented(
                    0,
                    format!("# Based on {:?}.", crate::format_path(source_path)),
                    2,
                );

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
                    format!("from {rerun_path}blueprint import Visualizer, VisualizableArchetype"),
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
