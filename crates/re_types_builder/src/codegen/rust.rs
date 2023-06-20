//! Implements the Rust codegen pass.

use anyhow::Context as _;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{StringExt as _, AUTOGEN_WARNING},
    ArrowRegistry, CodeGenerator, Docs, ElementType, Object, ObjectField, ObjectKind, Objects,
    Type, ATTR_RERUN_COMPONENT_OPTIONAL, ATTR_RERUN_COMPONENT_RECOMMENDED,
    ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RERUN_LEGACY_FQNAME, ATTR_RUST_DERIVE, ATTR_RUST_REPR,
    ATTR_RUST_TUPLE_STRUCT,
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
        filepaths.extend(create_files(
            datatypes_path,
            arrow_registry,
            &objects.ordered_objects(ObjectKind::Datatype.into()),
        ));

        let components_path = self.crate_path.join("src/components");
        std::fs::create_dir_all(&components_path)
            .with_context(|| format!("{components_path:?}"))
            .unwrap();
        filepaths.extend(create_files(
            components_path,
            arrow_registry,
            &objects.ordered_objects(ObjectKind::Component.into()),
        ));

        let archetypes_path = self.crate_path.join("src/archetypes");
        std::fs::create_dir_all(&archetypes_path)
            .with_context(|| format!("{archetypes_path:?}"))
            .unwrap();
        filepaths.extend(create_files(
            archetypes_path,
            arrow_registry,
            &objects.ordered_objects(ObjectKind::Archetype.into()),
        ));

        filepaths
    }
}

// --- File management ---

fn create_files(
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
            let tokens_str = obj.tokens.to_string();

            // NOTE: `TokenStream`s discard whitespacing information by definition, so we need to
            // inject some of our own when writing to file.
            let tokens_str = tokens_str
                .replace('}', "}\n\n")
                .replace("] ;", "];\n\n")
                .replace("# [doc", "\n\n#[doc")
                .replace("impl ", "\n\nimpl ");

            code.push_text(tokens_str, 1, 0);
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
    tokens: TokenStream,
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
            kind: _,
            attrs: _,
            fields,
            specifics: _,
            datatype: _,
        } = obj;

        let name = format_ident!("{name}");

        let quoted_doc = quote_doc_from_docs(docs);
        let quoted_derive_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE, "derive");
        let quoted_repr_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_REPR, "repr");

        let quoted_fields = fields
            .iter()
            .map(|obj_field| ObjectFieldTokenizer(obj, obj_field));

        let is_tuple_struct = is_tuple_struct_from_obj(obj);
        let quoted_struct = if is_tuple_struct {
            quote! { pub struct #name(#(#quoted_fields,)*); }
        } else {
            quote! { pub struct #name { #(#quoted_fields,)* } }
        };

        let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, obj);

        let quoted_builder = quote_builder_from_obj(obj);

        let tokens = quote! {
            #quoted_doc
            #quoted_derive_clause
            #quoted_repr_clause
            #quoted_struct

            #quoted_trait_impls

            #quoted_builder
        };

        Self {
            filepath: {
                let mut filepath = PathBuf::from(filepath);
                filepath.set_extension("rs");
                filepath
            },
            name: obj.name.clone(),
            tokens,
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
            datatype: _,
        } = obj;

        let name = format_ident!("{name}");

        let quoted_doc = quote_doc_from_docs(docs);
        let quoted_derive_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE, "derive");
        let quoted_repr_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_REPR, "repr");

        let quoted_fields = fields.iter().map(|obj_field| {
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
                datatype: _,
            } = obj_field;

            let name = format_ident!("{name}");

            let quoted_doc = quote_doc_from_docs(docs);
            let (quoted_type, _) = quote_field_type_from_field(obj_field, false);

            quote! {
                #quoted_doc
                #name(#quoted_type)
            }
        });

        let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, obj);

        let tokens = quote! {
            #quoted_doc
            #quoted_derive_clause
            #quoted_repr_clause
            pub enum #name {
                #(#quoted_fields,)*
            }

            #quoted_trait_impls
        };

        Self {
            filepath: {
                let mut filepath = PathBuf::from(filepath);
                filepath.set_extension("rs");
                filepath
            },
            name: obj.name.clone(),
            tokens,
        }
    }
}

// --- Code generators ---

struct ObjectFieldTokenizer<'a>(&'a Object, &'a ObjectField);

impl quote::ToTokens for ObjectFieldTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self(obj, obj_field) = self;

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
            datatype: _,
        } = obj_field;

        let quoted_docs = quote_doc_from_docs(docs);
        let name = format_ident!("{name}");

        let (quoted_type, _) = quote_field_type_from_field(obj_field, false);
        let quoted_type = if *required {
            quoted_type
        } else {
            quote!(Option<#quoted_type>)
        };

        if is_tuple_struct_from_obj(obj) {
            quote! {
                #quoted_docs
                pub #quoted_type
            }
        } else {
            quote! {
                #quoted_docs
                pub #name: #quoted_type
            }
        }
        .to_tokens(tokens);
    }
}

fn quote_doc_from_docs(docs: &Docs) -> TokenStream {
    struct DocCommentTokenizer<'a>(&'a [String]);

    impl quote::ToTokens for DocCommentTokenizer<'_> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.extend(self.0.iter().map(|line| quote!(#[doc = #line])));
        }
    }

    let lines = crate::codegen::get_documentation(docs, &["rs", "rust"]);
    let lines = DocCommentTokenizer(&lines);
    quote!(#lines)
}

/// Returns type name as string and whether it was force unwrapped.
///
/// Specifying `unwrap = true` will unwrap the final type before returning it, e.g. `Vec<String>`
/// becomes just `String`.
/// The returned boolean indicates whether there was anything to unwrap at all.
fn quote_field_type_from_field(obj_field: &ObjectField, unwrap: bool) -> (TokenStream, bool) {
    let obj_field_type = TypeTokenizer(&obj_field.typ, unwrap);
    let unwrapped = unwrap && matches!(obj_field.typ, Type::Array { .. } | Type::Vector { .. });
    (quote!(#obj_field_type), unwrapped)
}

/// `(type, unwrap)`
struct TypeTokenizer<'a>(&'a Type, bool);

impl quote::ToTokens for TypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self(typ, unwrap) = self;
        match typ {
            Type::UInt8 => quote!(u8),
            Type::UInt16 => quote!(u16),
            Type::UInt32 => quote!(u32),
            Type::UInt64 => quote!(u64),
            Type::Int8 => quote!(i8),
            Type::Int16 => quote!(i16),
            Type::Int32 => quote!(i32),
            Type::Int64 => quote!(i64),
            Type::Bool => quote!(bool),
            Type::Float16 => unimplemented!("{typ:#?}"), // NOLINT
            Type::Float32 => quote!(f32),
            Type::Float64 => quote!(f64),
            Type::String => quote!(String),
            Type::Array { elem_type, length } => {
                if *unwrap {
                    quote!(#elem_type)
                } else {
                    quote!([#elem_type; #length])
                }
            }
            Type::Vector { elem_type } => {
                if *unwrap {
                    quote!(#elem_type)
                } else {
                    quote!(Vec<#elem_type>)
                }
            }
            Type::Object(fqname) => quote_fqname_as_type_path(fqname),
        }
        .to_tokens(tokens);
    }
}

impl quote::ToTokens for &ElementType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            ElementType::UInt8 => quote!(u8),
            ElementType::UInt16 => quote!(u16),
            ElementType::UInt32 => quote!(u32),
            ElementType::UInt64 => quote!(u64),
            ElementType::Int8 => quote!(i8),
            ElementType::Int16 => quote!(i16),
            ElementType::Int32 => quote!(i32),
            ElementType::Int64 => quote!(i64),
            ElementType::Bool => quote!(bool),
            ElementType::Float16 => unimplemented!("{self:#?}"), // NOLINT
            ElementType::Float32 => quote!(f32),
            ElementType::Float64 => quote!(f64),
            ElementType::String => quote!(String),
            ElementType::Object(fqname) => quote_fqname_as_type_path(fqname),
        }
        .to_tokens(tokens);
    }
}

fn quote_meta_clause_from_obj(obj: &Object, attr: &str, clause: &str) -> TokenStream {
    let quoted = obj
        .try_get_attr::<String>(attr)
        .map(|what| {
            syn::parse_str::<syn::MetaList>(&format!("{clause}({what})"))
                .with_context(|| format!("illegal meta clause: {what:?}"))
                .unwrap()
        })
        .map(|clause| quote!(#[#clause]));
    quote!(#quoted)
}

fn quote_trait_impls_from_obj(arrow_registry: &ArrowRegistry, obj: &Object) -> TokenStream {
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
        datatype: _,
    } = obj;

    let name = format_ident!("{name}");

    match kind {
        ObjectKind::Datatype | ObjectKind::Component => {
            let kind = if *kind == ObjectKind::Datatype {
                quote!(Datatype)
            } else {
                quote!(Component)
            };
            let kind_name = format_ident!("{kind}Name");

            let datatype = arrow_registry.get(fqname);
            let datatype = ArrowDataTypeTokenizer(&datatype);

            let legacy_fqname = obj
                .try_get_attr::<String>(ATTR_RERUN_LEGACY_FQNAME)
                .unwrap_or_else(|| fqname.clone());

            quote! {
                impl crate::#kind for #name {
                    fn name() -> crate::#kind_name {
                        crate::#kind_name::Borrowed(#legacy_fqname)
                    }

                    #[allow(clippy::wildcard_imports)]
                    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
                        use ::arrow2::datatypes::*;
                        #datatype
                    }
                }
            }
        }
        ObjectKind::Archetype => {
            fn compute_components(obj: &Object, attr: &'static str) -> (usize, TokenStream) {
                let components = iter_archetype_components(obj, attr).collect::<Vec<_>>();
                let num_components = components.len();
                let quoted_components = quote!(#(crate::ComponentName::Borrowed(#components),)*);
                (num_components, quoted_components)
            }

            let (num_required, required) = compute_components(obj, ATTR_RERUN_COMPONENT_REQUIRED);
            let (num_recommended, recommended) =
                compute_components(obj, ATTR_RERUN_COMPONENT_RECOMMENDED);
            let (num_optional, optional) = compute_components(obj, ATTR_RERUN_COMPONENT_OPTIONAL);

            let num_all = num_required + num_recommended + num_optional;

            quote! {
                impl #name {
                    pub const REQUIRED_COMPONENTS: [crate::ComponentName; #num_required] = [#required];

                    pub const RECOMMENDED_COMPONENTS: [crate::ComponentName; #num_recommended] = [#recommended];

                    pub const OPTIONAL_COMPONENTS: [crate::ComponentName; #num_optional] = [#optional];

                    pub const ALL_COMPONENTS: [crate::ComponentName; #num_all] = [#required #recommended #optional];
                }

                impl crate::Archetype for #name {
                    fn name() -> crate::ArchetypeName {
                        crate::ArchetypeName::Borrowed(#fqname)
                    }

                    fn required_components() -> Vec<crate::ComponentName> {
                        Self::REQUIRED_COMPONENTS.to_vec()
                    }

                    fn recommended_components() -> Vec<crate::ComponentName> {
                        Self::RECOMMENDED_COMPONENTS.to_vec()
                    }

                    fn optional_components() -> Vec<crate::ComponentName> {
                        Self::OPTIONAL_COMPONENTS.to_vec()
                    }

                    #[allow(clippy::todo)]
                    fn to_arrow_datatypes() -> Vec<arrow2::datatypes::DataType> {
                        // TODO(#2368): dump the arrow registry into the generated code
                        todo!("query the registry for all fqnames");
                    }
                }
            }
        }
    }
}

/// Only makes sense for archetypes.
fn quote_builder_from_obj(obj: &Object) -> TokenStream {
    if obj.kind != ObjectKind::Archetype {
        return TokenStream::new();
    }

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
        datatype: _,
    } = obj;

    let name = format_ident!("{name}");

    // NOTE: Collecting because we need to iterate them more than once.
    let required = fields
        .iter()
        .filter(|field| field.required)
        .collect::<Vec<_>>();
    let optional = fields
        .iter()
        .filter(|field| !field.required)
        .collect::<Vec<_>>();

    // --- impl new() ---

    let quoted_params = required.iter().map(|field| {
        let field_name = format_ident!("{}", field.name);
        let (typ, unwrapped) = quote_field_type_from_field(field, true);
        if unwrapped {
            // This was originally a vec/array!
            quote!(#field_name: impl IntoIterator<Item = impl Into<#typ>>)
        } else {
            quote!(#field_name: impl Into<#typ>)
        }
    });

    let quoted_required = required.iter().map(|field| {
        let field_name = format_ident!("{}", field.name);
        let (_, unwrapped) = quote_field_type_from_field(field, true);
        if unwrapped {
            // This was originally a vec/array!
            quote!(#field_name: #field_name.into_iter().map(Into::into).collect())
        } else {
            quote!(#field_name: #field_name.into())
        }
    });

    let quoted_optional = optional.iter().map(|field| {
        let field_name = format_ident!("{}", field.name);
        quote!(#field_name: None)
    });

    let fn_new = quote! {
        pub fn new(#(#quoted_params,)*) -> Self {
            Self {
                #(#quoted_required,)*
                #(#quoted_optional,)*
            }
        }
    };

    // --- impl with_*() ---

    let with_methods = optional.iter().map(|field| {
        let field_name = format_ident!("{}", field.name);
        let method_name = format_ident!("with_{field_name}");
        let (typ, unwrapped) = quote_field_type_from_field(field, true);

        if unwrapped {
            // This was originally a vec/array!
            quote! {
                pub fn #method_name(mut self, #field_name: impl IntoIterator<Item = impl Into<#typ>>) -> Self {
                    self.#field_name = Some(#field_name.into_iter().map(Into::into).collect());
                    self
                }
            }
        } else {
            quote! {
                pub fn #method_name(mut self, #field_name: impl Into<#typ>) -> Self {
                    self.#field_name = Some(#field_name.into());
                    self
                }
            }
        }
    });

    quote! {
        impl #name {
            #fn_new

            #(#with_methods)*
        }
    }
}

// --- Arrow registry code generators ---

struct ArrowDataTypeTokenizer<'a>(&'a ::arrow2::datatypes::DataType);

impl quote::ToTokens for ArrowDataTypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use arrow2::datatypes::{DataType, UnionMode};
        match self.0 {
            DataType::Null => quote!(DataType::Null),
            DataType::Boolean => quote!(DataType::Boolean),
            DataType::Int8 => quote!(DataType::Int8),
            DataType::Int16 => quote!(DataType::Int16),
            DataType::Int32 => quote!(DataType::Int32),
            DataType::Int64 => quote!(DataType::Int64),
            DataType::UInt8 => quote!(DataType::UInt8),
            DataType::UInt16 => quote!(DataType::UInt16),
            DataType::UInt32 => quote!(DataType::UInt32),
            DataType::UInt64 => quote!(DataType::UInt64),
            DataType::Float16 => quote!(DataType::Float16),
            DataType::Float32 => quote!(DataType::Float32),
            DataType::Float64 => quote!(DataType::Float64),
            DataType::Binary => quote!(DataType::Binary),
            DataType::LargeBinary => quote!(DataType::LargeBinary),
            DataType::Utf8 => quote!(DataType::Utf8),
            DataType::LargeUtf8 => quote!(DataType::LargeUtf8),
            DataType::FixedSizeList(field, length) => {
                let field = ArrowFieldTokenizer(field);
                quote!(DataType::FixedSizeList(Box::new(#field), #length))
            }
            DataType::Union(fields, _, mode) => {
                let fields = fields.iter().map(ArrowFieldTokenizer);
                let mode = match mode {
                    UnionMode::Dense => quote!(UnionMode::Dense),
                    UnionMode::Sparse => quote!(UnionMode::Sparse),
                };
                quote!(DataType::Union(#(#fields,)*, None, #mode))
            }
            DataType::Struct(fields) => {
                let fields = fields.iter().map(ArrowFieldTokenizer);
                quote!(DataType::Struct(vec![ #(#fields,)* ]))
            }
            DataType::Extension(name, datatype, metadata) => {
                let datatype = ArrowDataTypeTokenizer(datatype);
                let metadata = OptionTokenizer(metadata.as_ref());
                quote!(DataType::Extension(#name.to_owned(), Box::new(#datatype), #metadata))
            }
            _ => unimplemented!("{:#?}", self.0), // NOLINT
        }
        .to_tokens(tokens);
    }
}

struct ArrowFieldTokenizer<'a>(&'a ::arrow2::datatypes::Field);

impl quote::ToTokens for ArrowFieldTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let arrow2::datatypes::Field {
            name,
            data_type,
            is_nullable,
            metadata,
        } = &self.0;

        let datatype = ArrowDataTypeTokenizer(data_type);
        let metadata = StrStrMapTokenizer(metadata);

        quote! {
            Field {
                name: #name.to_owned(),
                data_type: #datatype,
                is_nullable: #is_nullable,
                metadata: #metadata,
            }
        }
        .to_tokens(tokens);
    }
}

// NOTE: Needed because `quote!()` interprets the option otherwise.
struct OptionTokenizer<T>(Option<T>);

impl<T: quote::ToTokens> quote::ToTokens for OptionTokenizer<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(v) = &self.0 {
            quote!(Some(#v))
        } else {
            quote!(None)
        }
        .to_tokens(tokens);
    }
}

struct StrStrMapTokenizer<'a>(&'a std::collections::BTreeMap<String, String>);

impl quote::ToTokens for StrStrMapTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let k = self.0.keys();
        let v = self.0.values();
        quote!([#((#k, #v),)*].into()).to_tokens(tokens);
    }
}

fn quote_fqname_as_type_path(fqname: impl AsRef<str>) -> TokenStream {
    let fqname = fqname.as_ref().replace('.', "::").replace("rerun", "crate");
    let expr: syn::TypePath = syn::parse_str(&fqname).unwrap();
    quote!(#expr)
}

// --- Helpers ---

fn is_tuple_struct_from_obj(obj: &Object) -> bool {
    obj.is_struct()
        && obj.fields.len() == 1
        && obj.try_get_attr::<String>(ATTR_RUST_TUPLE_STRUCT).is_some()
}

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
