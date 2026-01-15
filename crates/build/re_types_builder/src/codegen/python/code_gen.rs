//! Main code generation functions for Python types.

use std::collections::{BTreeSet, HashMap};

use itertools::Itertools as _;
use unindent::unindent;

use super::archetype_methods::{quote_clear_methods, quote_columnar_methods, quote_partial_update_methods};
use super::arrow::quote_arrow_support_from_obj;
use super::docs::{lines_from_docs, quote_doc_from_fields, quote_doc_lines, quote_obj_docs, quote_union_kind_from_fields};
use super::extension_class::{ExtensionClass, ExtensionClasses, FIELD_CONVERTER_SUFFIX};
use super::init_method::quote_init_method;
use super::object_ext::PythonObjectExt;
use super::typing::{quote_aliases_from_object, quote_field_converter_from_field, quote_field_type_from_field, quote_union_aliases_from_object};
use super::classmethod_decorators;
use crate::codegen::StringExt as _;
use crate::objects::ObjectClass;
use crate::{Object, ObjectField, ObjectKind, Objects, Reporter, Type, TypeRegistry};

pub fn code_for_struct(
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
                code.push_indented(1, "", 1);
                code.push_indented(1, "def visualizer(self) -> Visualizer:", 1);
                code.push_indented(2, r#""""Creates a visualizer for this archetype.""""#, 1);
                // TODO(RR-3254): Add options for mapping here
                code.push_indented(2, format!(r#"return Visualizer("{visualizer_name}", overrides=self.as_component_batches(), mappings=None)"#), 1);
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

pub fn code_for_enum(
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

pub fn code_for_union(
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
