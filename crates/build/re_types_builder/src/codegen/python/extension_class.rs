//! Extension class handling for Python codegen.

use std::collections::{BTreeMap, HashSet};
use std::iter;
use std::ops::Deref;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use regex_lite::Regex;

use crate::{Object, ObjectKind, Objects, Reporter, Type};

/// The standard python init method.
pub const INIT_METHOD: &str = "__init__";

/// The standard numpy interface for converting to an array type
pub const ARRAY_METHOD: &str = "__array__";

/// The standard python len method
pub const LEN_METHOD: &str = "__len__";

/// The method used to convert a native type into a pyarrow array
pub const NATIVE_TO_PA_ARRAY_METHOD: &str = "native_to_pa_array_override";

/// The method used for deferred patch class init.
/// Use this for initialization constants that need to know the child (non-extension) class.
pub const DEFERRED_PATCH_CLASS_METHOD: &str = "deferred_patch_class";

/// The common suffix for method used to convert fields to their canonical representation.
pub const FIELD_CONVERTER_SUFFIX: &str = "__field_converter_override";

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
pub struct ExtensionClass {
    /// Whether or not the `ObjectExt` was found
    pub found: bool,

    /// The name of the file where the `ObjectExt` is implemented
    pub file_name: String,

    /// The name of the module where `ObjectExt` is implemented
    pub module_name: String,

    /// The name of this `ObjectExt`
    pub name: String,

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
    pub field_converter_overrides: Vec<String>,

    /// Whether the `ObjectExt` contains __init__()
    ///
    /// If the `ExtensioNClass` contains its own `__init__`, we need to avoid creating the
    /// default `__init__` via `attrs.define`. This can be done by specifying:
    /// ```python
    /// @define(init=false)
    /// ```
    pub has_init: bool,

    /// Whether the `ObjectExt` contains __array__()
    ///
    /// If the `ExtensionClass` contains its own `__array__` then we avoid generating
    /// a default implementation.
    pub has_array: bool,

    /// Whether the `ObjectExt` contains `__native_to_pa_array__()`
    pub has_native_to_pa_array: bool,

    /// Whether the `ObjectExt` contains a `deferred_patch_class()` method
    pub has_deferred_patch_class: bool,

    /// Whether the `ObjectExt` contains __len__()
    ///
    /// If the `ExtensionClass` contains its own `__len__` then we avoid generating
    /// a default implementation.
    pub has_len: bool,
}

impl ExtensionClass {
    pub fn new(reporter: &Reporter, base_path: &Utf8Path, obj: &Object, objects: &Objects) -> Self {
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

pub struct ExtensionClasses {
    pub classes: BTreeMap<String, ExtensionClass>,
}

impl Deref for ExtensionClasses {
    type Target = BTreeMap<String, ExtensionClass>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.classes
    }
}
