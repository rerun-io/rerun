//! Implements the Rust codegen pass.

use anyhow::Context as _;
use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{StringExt as _, AUTOGEN_WARNING},
    ArrowRegistry, CodeGenerator, Docs, ElementType, Object, ObjectField, ObjectKind, Objects,
    Type, ATTR_RERUN_COMPONENT_OPTIONAL, ATTR_RERUN_COMPONENT_RECOMMENDED,
    ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RUST_DERIVE, ATTR_RUST_REPR, ATTR_RUST_TUPLE_STRUCT,
};

// ---

pub struct RustCodeGenerator {
    crate_path: PathBuf,
}

impl RustCodeGenerator {
    pub fn new(crate_path: impl Into<PathBuf>) -> Self {
        Self {
            crate_path: crate_path.into(),
        }
    }
}

impl CodeGenerator for RustCodeGenerator {
    fn generate(&mut self, objects: &Objects, arrow_registry: &ArrowRegistry) -> Vec<PathBuf> {
        let mut filepaths = Vec::new();

        let datatypes_path = self.crate_path.join("src/datatypes");
        std::fs::create_dir_all(&datatypes_path)
            .with_context(|| format!("{datatypes_path:?}"))
            .unwrap();
        filepaths.extend(quote_objects(
            datatypes_path,
            arrow_registry,
            &objects.ordered_datatypes(),
        ));

        let components_path = self.crate_path.join("src/components");
        std::fs::create_dir_all(&components_path)
            .with_context(|| format!("{components_path:?}"))
            .unwrap();
        filepaths.extend(quote_objects(
            components_path,
            arrow_registry,
            &objects.ordered_components(),
        ));

        let archetypes_path = self.crate_path.join("src/archetypes");
        std::fs::create_dir_all(&archetypes_path)
            .with_context(|| format!("{archetypes_path:?}"))
            .unwrap();
        filepaths.extend(quote_objects(
            archetypes_path,
            arrow_registry,
            &objects.ordered_archetypes(),
        ));

        filepaths
    }
}

// --- File management ---

fn quote_objects(
    out_path: impl AsRef<Path>,
    arrow_registry: &ArrowRegistry,
    objs: &[&Object],
) -> Vec<PathBuf> {
    let out_path = out_path.as_ref();

    let mut filepaths = Vec::new();

    let mut files = HashMap::<PathBuf, Vec<QuotedObject>>::new();
    for obj in objs {
        let obj = if obj.is_struct() {
            QuotedObject::from_struct(arrow_registry, obj)
        } else {
            QuotedObject::from_union(arrow_registry, obj)
        };

        let filepath = out_path.join(obj.filepath.file_name().unwrap());
        files.entry(filepath.clone()).or_default().push(obj);
    }

    // (module_name, [object_name])
    let mut mods = HashMap::<String, Vec<String>>::new();

    // src/{datatypes|components|archetypes}/{xxx}.rs
    for (filepath, objs) in files {
        // NOTE: Isolating the file stem only works because we're handling datatypes, components
        // and archetypes separately (and even then it's a bit shady, eh).
        let names = objs.iter().map(|obj| obj.name.clone()).collect::<Vec<_>>();
        mods.entry(filepath.file_stem().unwrap().to_string_lossy().to_string())
            .or_default()
            .extend(names);

        filepaths.push(filepath.clone());
        let mut file = std::fs::File::create(&filepath)
            .with_context(|| format!("{filepath:?}"))
            .unwrap();

        let mut code = String::new();
        code.push_text(format!("// {AUTOGEN_WARNING}"), 2, 0);

        for obj in objs {
            code.push_text(&obj.code, 1, 0);
        }
        file.write_all(code.as_bytes())
            .with_context(|| format!("{filepath:?}"))
            .unwrap();
    }

    // src/{datatypes|components|archetypes}/mod.rs
    {
        let path = out_path.join("mod.rs");

        let mut code = String::new();

        code.push_text(format!("// {AUTOGEN_WARNING}"), 2, 0);

        for module in mods.keys() {
            code.push_text(format!("mod {module};"), 1, 0);

            // Detect if someone manually created an extension file, and automatically
            // import it if so.
            let mut ext_path = out_path.join(format!("{module}_ext"));
            ext_path.set_extension("rs");
            if ext_path.exists() {
                code.push_text(format!("mod {module}_ext;"), 1, 0);
            }
        }

        code += "\n\n";

        for (module, names) in &mods {
            let names = names.join(", ");
            code.push_text(format!("pub use self::{module}::{{{names}}};"), 1, 0);
        }

        filepaths.push(path.clone());
        std::fs::write(&path, code)
            .with_context(|| format!("{path:?}"))
            .unwrap();
    }

    filepaths
}

// --- Codegen core loop ---

#[derive(Debug, Clone)]
struct QuotedObject {
    filepath: PathBuf,
    name: String,
    code: String,
}

impl QuotedObject {
    fn from_struct(arrow_registry: &ArrowRegistry, obj: &Object) -> Self {
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

        code.push_text(&quote_doc_from_docs(docs), 0, 0);

        if let Some(clause) = quote_derive_clause_from_obj(obj) {
            code.push_text(&clause, 1, 0);
        }
        if let Some(clause) = quote_repr_clause_from_obj(obj) {
            code.push_text(&clause, 1, 0);
        }

        let is_tuple_struct = is_tuple_struct_from_obj(obj);

        if is_tuple_struct {
            code.push_text(&format!("pub struct {name}("), 0, 0);
        } else {
            code.push_text(&format!("pub struct {name} {{"), 1, 0);
        }

        for field in fields {
            let ObjectField {
                filepath: _,
                pkg_name: _,
                fqname: _,
                name,
                docs,
                typ: _,
                attrs: _,
                required,
                // TODO(#2366): support for deprecation notices
                deprecated: _,
            } = field;

            code.push_text(&quote_doc_from_docs(docs), 0, 0);

            let (typ, _) = quote_field_type_from_field(field, false);
            let typ = if *required {
                typ
            } else {
                format!("Option<{typ}>")
            };

            if is_tuple_struct {
                code.push_text(&format!("pub {typ}"), 0, 0);
            } else {
                code.push_text(&format!("pub {name}: {typ},"), 2, 0);
            }
        }

        if is_tuple_struct {
            code += ");\n\n";
        } else {
            code += "}\n\n";
        }

        code.push_text(&quote_trait_impls_from_obj(arrow_registry, obj), 1, 0);

        if kind == &ObjectKind::Archetype {
            code.push_text(&quote_builder_from_obj(obj), 0, 0);
        }

        let mut filepath = PathBuf::from(filepath);
        filepath.set_extension("rs");

        Self {
            filepath,
            name: obj.name.clone(),
            code,
        }
    }

    fn from_union(arrow_registry: &ArrowRegistry, obj: &Object) -> Self {
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

        code.push_text(&quote_doc_from_docs(docs), 0, 0);

        if let Some(clause) = quote_derive_clause_from_obj(obj) {
            code.push_text(&clause, 1, 0);
        }
        if let Some(clause) = quote_repr_clause_from_obj(obj) {
            code.push_text(&clause, 1, 0);
        }

        code.push_text(&format!("pub enum {name} {{"), 1, 0);

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

            code.push_text(&quote_doc_from_docs(docs), 0, 0);

            let (typ, _) = quote_field_type_from_field(field, false);

            code.push_text(&format!("{name}({typ}),"), 2, 0);
        }

        code += "}\n\n";

        code.push_text(&quote_trait_impls_from_obj(arrow_registry, obj), 1, 0);

        let mut filepath = PathBuf::from(filepath);
        filepath.set_extension("rs");

        Self {
            filepath,
            name: obj.name.clone(),
            code,
        }
    }
}

// --- Code generators ---

fn quote_doc_from_docs(docs: &Docs) -> String {
    let lines = crate::codegen::quote_doc_from_docs(docs, &["rs", "rust"]);
    let lines = lines
        .into_iter()
        .map(|line| format!("/// {line}"))
        .collect::<Vec<_>>();

    let mut doc = lines.join("\n");
    doc.push('\n');
    doc
}

/// Returns type name as string and whether it was force unwrapped.
///
/// Specifying `unwrap = true` will unwrap the final type before returning it, e.g. `Vec<String>`
/// becomes just `String`.
/// The returned boolean indicates whether there was anything to unwrap at all.
fn quote_field_type_from_field(field: &ObjectField, unwrap: bool) -> (String, bool) {
    let mut unwrapped = false;
    let typ = &field.typ;
    let typ = match typ {
        Type::UInt8 => "u8".to_owned(),
        Type::UInt16 => "u16".to_owned(),
        Type::UInt32 => "u32".to_owned(),
        Type::UInt64 => "u64".to_owned(),
        Type::Int8 => "i8".to_owned(),
        Type::Int16 => "i16".to_owned(),
        Type::Int32 => "i32".to_owned(),
        Type::Int64 => "i64".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::Float16 => unimplemented!("{typ:#?}"), // NOLINT
        Type::Float32 => "f32".to_owned(),
        Type::Float64 => "f64".to_owned(),
        Type::String => "String".to_owned(),
        Type::Array { elem_type, length } => {
            let typ = quote_type_from_element_type(elem_type);
            if unwrap {
                unwrapped = true;
                typ
            } else {
                format!("[{typ}; {length}]")
            }
        }
        Type::Vector { elem_type } => {
            let typ = quote_type_from_element_type(elem_type);
            if unwrap {
                unwrapped = true;
                typ
            } else {
                format!("Vec<{typ}>")
            }
        }
        Type::Object(fqname) => fqname.replace('.', "::").replace("rerun", "crate"),
    };

    (typ, unwrapped)
}

fn quote_type_from_element_type(typ: &ElementType) -> String {
    match typ {
        ElementType::UInt8 => "u8".to_owned(),
        ElementType::UInt16 => "u16".to_owned(),
        ElementType::UInt32 => "u32".to_owned(),
        ElementType::UInt64 => "u64".to_owned(),
        ElementType::Int8 => "i8".to_owned(),
        ElementType::Int16 => "i16".to_owned(),
        ElementType::Int32 => "i32".to_owned(),
        ElementType::Int64 => "i64".to_owned(),
        ElementType::Bool => "bool".to_owned(),
        ElementType::Float16 => unimplemented!("{typ:#?}"), // NOLINT
        ElementType::Float32 => "f32".to_owned(),
        ElementType::Float64 => "f64".to_owned(),
        ElementType::String => "String".to_owned(),
        ElementType::Object(fqname) => fqname.replace('.', "::").replace("rerun", "crate"),
    }
}

fn quote_derive_clause_from_obj(obj: &Object) -> Option<String> {
    obj.try_get_attr::<String>(ATTR_RUST_DERIVE)
        .map(|what| format!("#[derive({what})]"))
}

fn quote_repr_clause_from_obj(obj: &Object) -> Option<String> {
    obj.try_get_attr::<String>(ATTR_RUST_REPR)
        .map(|what| format!("#[repr({what})]"))
}

fn is_tuple_struct_from_obj(obj: &Object) -> bool {
    obj.is_struct()
        && obj.fields.len() == 1
        && obj.try_get_attr::<String>(ATTR_RUST_TUPLE_STRUCT).is_some()
}

fn quote_trait_impls_from_obj(arrow_registry: &ArrowRegistry, obj: &Object) -> String {
    let Object {
        filepath: _,
        fqname,
        pkg_name: _,
        name,
        docs: _,
        kind,
        attrs: _,
        fields: _,
        specifics: _,
    } = obj;

    match kind {
        ObjectKind::Datatype => {
            let datatype = quote_arrow_datatype(&arrow_registry.get(fqname));
            format!(
                r#"
                impl crate::Datatype for {name} {{
                    fn name() -> crate::DatatypeName {{
                        crate::DatatypeName::Borrowed({fqname:?})
                    }}

                    #[allow(clippy::wildcard_imports)]
                    fn to_arrow_datatype() -> arrow2::datatypes::DataType {{
                        use ::arrow2::datatypes::*;
                        {datatype}
                    }}
                }}
                "#
            )
        }
        ObjectKind::Component => {
            let datatype = quote_arrow_datatype(&arrow_registry.get(fqname));
            format!(
                r#"
                impl crate::Component for {name} {{
                    fn name() -> crate::ComponentName {{
                        crate::ComponentName::Borrowed({fqname:?})
                    }}

                    #[allow(clippy::wildcard_imports)]
                    fn to_arrow_datatype() -> arrow2::datatypes::DataType {{
                        use ::arrow2::datatypes::*;
                        {datatype}
                    }}
                }}
                "#
            )
        }
        ObjectKind::Archetype => {
            fn compute_components(obj: &Object, attr: &'static str) -> (usize, String) {
                let components = iter_archetype_components(obj, attr).collect::<Vec<_>>();

                let num_components = components.len();
                let components = components
                    .into_iter()
                    .map(|fqname| format!("crate::ComponentName::Borrowed({fqname:?})"))
                    .collect::<Vec<_>>()
                    .join(", ");

                (num_components, components)
            }

            let (num_required, required) = compute_components(obj, ATTR_RERUN_COMPONENT_REQUIRED);
            let (num_recommended, recommended) =
                compute_components(obj, ATTR_RERUN_COMPONENT_RECOMMENDED);
            let (num_optional, optional) = compute_components(obj, ATTR_RERUN_COMPONENT_OPTIONAL);

            let num_all = num_required + num_recommended + num_optional;
            let all = [required.as_str(), recommended.as_str(), optional.as_str()]
                .as_slice()
                .join(", ");

            format!(
                r#"
                impl {name} {{
                    pub const REQUIRED_COMPONENTS: [crate::ComponentName; {num_required}] = [{required}];

                    pub const RECOMMENDED_COMPONENTS: [crate::ComponentName; {num_recommended}] = [{recommended}];

                    pub const OPTIONAL_COMPONENTS: [crate::ComponentName; {num_optional}] = [{optional}];

                    pub const ALL_COMPONENTS: [crate::ComponentName; {num_all}] = [{all}];
                }}

                impl crate::Archetype for {name} {{
                    fn name() -> crate::ArchetypeName {{
                        crate::ArchetypeName::Borrowed({fqname:?})
                    }}

                    fn required_components() -> Vec<crate::ComponentName> {{
                        Self::REQUIRED_COMPONENTS.to_vec()
                    }}

                    fn recommended_components() -> Vec<crate::ComponentName> {{
                        Self::RECOMMENDED_COMPONENTS.to_vec()
                    }}

                    fn optional_components() -> Vec<crate::ComponentName> {{
                        Self::OPTIONAL_COMPONENTS.to_vec()
                    }}

                    #[allow(clippy::unimplemented)]
                    fn to_arrow_datatypes() -> Vec<arrow2::datatypes::DataType> {{
                        // TODO(#2368): dump the arrow registry into the generated code
                        unimplemented!("query the registry for all fqnames"); // NOLINT
                    }}
                }}
            "#
            )
        }
    }
}

/// Only makes sense for archetypes.
fn quote_builder_from_obj(obj: &Object) -> String {
    assert_eq!(ObjectKind::Archetype, obj.kind);

    let Object {
        filepath: _,
        fqname: _,
        pkg_name: _,
        name,
        docs: _,
        kind: _,
        attrs: _,
        fields,
        specifics: _,
    } = obj;

    let required = fields
        .iter()
        .filter(|field| field.required)
        .collect::<Vec<_>>();
    let optional = fields
        .iter()
        .filter(|field| !field.required)
        .collect::<Vec<_>>();

    let mut code = String::new();

    code.push_text(&format!("impl {name} {{"), 1, 0);
    {
        // --- impl new() ---

        let new_params = required
            .iter()
            .map(|field| {
                let (typ, unwrapped) = quote_field_type_from_field(field, true);
                if unwrapped {
                    // This was originally a vec/array!
                    format!(
                        "{}: impl IntoIterator<Item = impl Into<{}>>",
                        field.name, typ
                    )
                } else {
                    format!("{}: impl Into<{}>", field.name, typ)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        code.push_text(&format!("pub fn new({new_params}) -> Self {{"), 1, 0);
        {
            code += "Self {\n";
            {
                for field in &required {
                    let (_, unwrapped) = quote_field_type_from_field(field, true);
                    if unwrapped {
                        // This was originally a vec/array!
                        code.push_text(
                            &format!(
                                "{}: {}.into_iter().map(Into::into).collect(),",
                                field.name, field.name
                            ),
                            1,
                            0,
                        );
                    } else {
                        code.push_text(&format!("{}: {}.into(),", field.name, field.name), 1, 0);
                    }
                }
                for field in &optional {
                    code.push_text(&format!("{}: None,", field.name), 1, 0);
                }
            }
            code += "}\n";
        }
        code += "}\n\n";

        // --- impl with_*() ---

        for field in &optional {
            let name = &field.name;
            let (typ, unwrapped) = quote_field_type_from_field(field, true);

            if unwrapped {
                // This was originally a vec/array!
                code.push_text(&format!(
                    "pub fn with_{name}(mut self, {name}: impl IntoIterator<Item = impl Into<{typ}>>) -> Self {{",
                ), 1, 0);
                {
                    code.push_text(
                        &format!(
                            "self.{name} = Some({name}.into_iter().map(Into::into).collect());"
                        ),
                        1,
                        0,
                    );
                    code += "self\n";
                }
            } else {
                code.push_text(
                    &format!("pub fn with_{name}(mut self, {name}: impl Into<{typ}>) -> Self {{",),
                    1,
                    0,
                );
                {
                    code.push_text(&format!("self.{name} = Some({name}.into());"), 1, 0);
                    code += "self\n";
                }
            }

            code += "}\n\n";
        }
    }
    code += "}\n\n";

    code
}

// --- Arrow registry code generators ---

use arrow2::datatypes::{DataType, Field};

fn quote_arrow_datatype(datatype: &DataType) -> String {
    match datatype {
        DataType::Null => "DataType::Null".to_owned(),
        DataType::Boolean => "DataType::Boolean".to_owned(),
        DataType::Int8 => "DataType::Int8".to_owned(),
        DataType::Int16 => "DataType::Int16".to_owned(),
        DataType::Int32 => "DataType::Int32".to_owned(),
        DataType::Int64 => "DataType::Int64".to_owned(),
        DataType::UInt8 => "DataType::UInt8".to_owned(),
        DataType::UInt16 => "DataType::UInt16".to_owned(),
        DataType::UInt32 => "DataType::UInt32".to_owned(),
        DataType::UInt64 => "DataType::UInt64".to_owned(),
        DataType::Float16 => "DataType::Float16".to_owned(),
        DataType::Float32 => "DataType::Float32".to_owned(),
        DataType::Float64 => "DataType::Float64".to_owned(),
        DataType::Date32 => "DataType::Date32".to_owned(),
        DataType::Date64 => "DataType::Date64".to_owned(),
        DataType::Binary => "DataType::Binary".to_owned(),
        DataType::LargeBinary => "DataType::LargeBinary".to_owned(),
        DataType::Utf8 => "DataType::Utf8".to_owned(),
        DataType::LargeUtf8 => "DataType::LargeUtf8".to_owned(),
        DataType::FixedSizeList(field, length) => {
            let field = quote_arrow_field(field);
            format!("DataType::FixedSizeList(Box::new({field}), {length})")
        }
        DataType::Union(fields, _, mode) => {
            let fields = fields
                .iter()
                .map(quote_arrow_field)
                .collect::<Vec<_>>()
                .join(", ");

            // NOTE: unindenting to work around a rustfmt bug
            unindent::unindent(&format!(
                r#"
                DataType::Union(
                    vec![{fields}],
                    None,
                    UnionMode::{mode:?},
                )
                "#
            ))
        }
        DataType::Struct(fields) => {
            let fields = fields
                .iter()
                .map(quote_arrow_field)
                .collect::<Vec<_>>()
                .join(", ");

            format!("DataType::Struct(vec![{fields}])")
        }
        DataType::Extension(name, datatype, metadata) => {
            let datatype = quote_arrow_datatype(datatype);
            let metadata = quote_optional_string(metadata.as_deref());

            // NOTE: unindenting to work around a rustfmt bug
            unindent::unindent(&format!(
                r#"
                DataType::Extension(
                    "{name}".to_owned(),
                    Box::new({datatype}),
                    {metadata},
                )
                "#
            ))
        }
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
    let metadata = quote_metadata_map(metadata);

    // NOTE: unindenting to work around a rustfmt bug
    unindent::unindent(&format!(
        r#"
        Field {{
            name: "{name}".to_owned(),
            data_type: {datatype},
            is_nullable: {is_nullable},
            metadata: {metadata},
        }}
        "#
    ))
}

fn quote_optional_string(s: Option<&str>) -> String {
    if let Some(s) = s {
        format!("Some({s:?})")
    } else {
        "None".into()
    }
}

fn quote_metadata_map(metadata: &BTreeMap<String, String>) -> String {
    let kvs = metadata
        .iter()
        .map(|(k, v)| format!("({k:?}, {v:?})"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{kvs}].into()")
}

// --- Helpers ---

fn iter_archetype_components<'a>(
    obj: &'a Object,
    requirement_attr_value: &'static str,
) -> impl Iterator<Item = String> + 'a {
    assert_eq!(ObjectKind::Archetype, obj.kind);
    obj.fields.iter().filter_map(move |field| {
        field
            .try_get_attr::<String>(requirement_attr_value)
            .map(|_| match &field.typ {
                Type::Object(fqname) => fqname.clone(),
                Type::Vector { elem_type } => match elem_type {
                    ElementType::Object(fqname) => fqname.clone(),
                    _ => {
                        panic!("archetype field must be an object/union or an array/vector of such")
                    }
                },
                _ => panic!("archetype field must be an object/union or an array/vector of such"),
            })
    })
}
