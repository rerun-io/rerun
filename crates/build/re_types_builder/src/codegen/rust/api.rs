use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    codegen::{
        autogen_warning,
        rust::{
            arrow::ArrowDataTypeTokenizer,
            deserializer::{
                quote_arrow_deserializer, quote_arrow_deserializer_buffer_slice,
                should_optimize_buffer_slice_deserialize,
            },
            serializer::quote_arrow_serializer,
            util::{is_tuple_struct_from_obj, quote_doc_line},
        },
        Target,
    },
    format_path,
    objects::ObjectClass,
    ArrowRegistry, CodeGenerator, ElementType, Object, ObjectField, ObjectKind, Objects, Reporter,
    Type, ATTR_DEFAULT, ATTR_RERUN_COMPONENT_OPTIONAL, ATTR_RERUN_COMPONENT_RECOMMENDED,
    ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RERUN_LOG_MISSING_AS_EMPTY, ATTR_RERUN_VIEW_IDENTIFIER,
    ATTR_RUST_CUSTOM_CLAUSE, ATTR_RUST_DERIVE, ATTR_RUST_DERIVE_ONLY, ATTR_RUST_NEW_PUB_CRATE,
    ATTR_RUST_REPR,
};

use super::{
    arrow::quote_fqname_as_type_path,
    blueprint_validation::generate_blueprint_validation,
    reflection::generate_reflection,
    util::{append_tokens, doc_as_lines, quote_doc_lines},
};

// ---

pub struct RustCodeGenerator {
    pub workspace_path: Utf8PathBuf,
}

impl RustCodeGenerator {
    pub fn new(workspace_path: impl Into<Utf8PathBuf>) -> Self {
        let workspace_path = workspace_path.into();
        Self { workspace_path }
    }
}

impl CodeGenerator for RustCodeGenerator {
    fn generate(
        &mut self,
        reporter: &Reporter,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
    ) -> BTreeMap<Utf8PathBuf, String> {
        let mut files_to_write: BTreeMap<Utf8PathBuf, String> = Default::default();
        let mut extension_contents_for_fqname: HashMap<String, String> = Default::default();

        for object_kind in ObjectKind::ALL {
            self.generate_folder(
                reporter,
                objects,
                arrow_registry,
                object_kind,
                &mut files_to_write,
                &mut extension_contents_for_fqname,
            );
        }

        generate_blueprint_validation(reporter, objects, &mut files_to_write);
        generate_reflection(
            reporter,
            objects,
            &extension_contents_for_fqname,
            &mut files_to_write,
        );

        files_to_write
    }
}

impl RustCodeGenerator {
    fn generate_folder(
        &self,
        reporter: &Reporter,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
        object_kind: ObjectKind,
        files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
        extension_contents_for_fqname: &mut HashMap<String, String>,
    ) {
        let crates_root_path = self.workspace_path.join("crates");

        let mut all_modules: HashSet<_> = HashSet::default();

        // Generate folder contents:
        for obj in objects.objects_of_kind(object_kind) {
            let crate_name = obj.crate_name();
            let module_name = obj.module_name();

            let crate_path = crates_root_path.join("store").join(&crate_name);
            let module_path = if obj.is_testing() {
                crate_path.join("src/testing").join(&module_name)
            } else {
                crate_path.join("src").join(&module_name)
            };

            let filename_stem = obj.snake_case_name();
            let filename = format!("{filename_stem}.rs");

            let filepath = module_path.join(filename);
            let mut code = generate_object_file(reporter, objects, arrow_registry, obj, &filepath);

            if let Ok(extension_contents) =
                std::fs::read_to_string(module_path.join(format!("{filename_stem}_ext.rs")))
            {
                extension_contents_for_fqname.insert(obj.fqname.clone(), extension_contents);
            }

            if crate_name == "re_types_core" {
                code = code.replace("::re_types_core", "crate");
            }

            all_modules.insert((
                crate_name,
                module_name,
                obj.is_testing(),
                module_path.clone(),
            ));
            files_to_write.insert(filepath, code);
        }

        for (crate_name, module_name, is_testing, module_path) in all_modules {
            let relevant_objs = objects
                .objects_of_kind(object_kind)
                .filter(|obj| obj.is_testing() == is_testing)
                .filter(|obj| obj.crate_name() == crate_name)
                .filter(|obj| obj.module_name() == module_name)
                .collect_vec();

            // src/{testing/}{datatypes|components|archetypes}/mod.rs
            generate_mod_file(&module_path, &relevant_objs, files_to_write);
        }
    }
}

fn generate_object_file(
    reporter: &Reporter,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
    obj: &Object,
    target_file: &Utf8Path,
) -> String {
    let mut code = String::new();
    code.push_str(&format!("// {}\n", autogen_warning!()));
    if let Some(source_path) = obj.relative_filepath() {
        code.push_str(&format!("// Based on {:?}.\n\n", format_path(source_path)));
    }

    code.push_str("#![allow(unused_imports)]\n");
    code.push_str("#![allow(unused_parens)]\n");
    code.push_str("#![allow(clippy::clone_on_copy)]\n");
    code.push_str("#![allow(clippy::cloned_instead_of_copied)]\n");
    code.push_str("#![allow(clippy::map_flatten)]\n");
    code.push_str("#![allow(clippy::needless_question_mark)]\n");
    code.push_str("#![allow(clippy::new_without_default)]\n");
    code.push_str("#![allow(clippy::redundant_closure)]\n");
    code.push_str("#![allow(clippy::too_many_arguments)]\n"); // e.g. `AffixFuzzer1::new`
    code.push_str("#![allow(clippy::too_many_lines)]\n");
    if obj.deprecation_notice().is_some() {
        code.push_str("#![allow(deprecated)]\n");
    }

    if obj.is_enum() {
        // Needed for PixelFormat. Should we limit this via attribute to just that?
        code.push_str("#![allow(non_camel_case_types)]\n");
    }

    code.push_str("\n\n");

    code.push_str("use ::re_types_core::external::arrow2;\n");
    code.push_str("use ::re_types_core::SerializationResult;\n");
    code.push_str("use ::re_types_core::{DeserializationResult, DeserializationError};\n");
    code.push_str("use ::re_types_core::{ComponentDescriptor, ComponentName};\n");
    code.push_str("use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};\n");

    // NOTE: `TokenStream`s discard whitespacing information by definition, so we need to
    // inject some of our own when writing to file… while making sure that don't inject
    // random spacing into doc comments that look like code!

    let quoted_obj = match obj.class {
        crate::objects::ObjectClass::Struct => quote_struct(reporter, arrow_registry, objects, obj),
        crate::objects::ObjectClass::Union => quote_union(reporter, arrow_registry, objects, obj),
        crate::objects::ObjectClass::Enum => quote_enum(reporter, arrow_registry, objects, obj),
    };

    append_tokens(reporter, code, &quoted_obj, target_file)
}

fn generate_mod_file(
    dirpath: &Utf8Path,
    objects: &[&Object],
    files_to_write: &mut BTreeMap<Utf8PathBuf, String>,
) {
    let path = dirpath.join("mod.rs");

    let mut code = String::new();

    code.push_str(&format!("// {}\n\n", autogen_warning!()));

    for obj in objects {
        let module_name = obj.snake_case_name();
        code.push_str(&format!("mod {module_name};\n"));

        // Detect if someone manually created an extension file, and automatically
        // import it if so.
        let mut ext_path = dirpath.join(format!("{module_name}_ext"));
        ext_path.set_extension("rs");
        if ext_path.exists() {
            code.push_str(&format!("mod {module_name}_ext;\n"));
        }
    }

    code.push_str("\n\n");

    // Non-deprecated first.
    for obj in objects
        .iter()
        .filter(|obj| obj.deprecation_notice().is_none())
    {
        let module_name = obj.snake_case_name();
        let type_name = &obj.name;
        code.push_str(&format!("pub use self::{module_name}::{type_name};\n"));
    }
    // And then deprecated.
    if objects.iter().any(|obj| obj.deprecation_notice().is_some()) {
        code.push_str("\n\n");
    }
    for obj in objects
        .iter()
        .filter(|obj| obj.deprecation_notice().is_some())
    {
        let module_name = obj.snake_case_name();
        let type_name = &obj.name;
        if obj.deprecation_notice().is_some() {
            code.push_str("#[allow(deprecated)]\n");
        }
        code.push_str(&format!("pub use self::{module_name}::{type_name};\n"));
    }

    files_to_write.insert(path, code);
}

// --- Codegen core loop ---

fn quote_struct(
    reporter: &Reporter,
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    assert!(obj.is_struct());

    let Object { name, fields, .. } = obj;

    let name = format_ident!("{name}");

    let quoted_doc = quote_obj_docs(reporter, objects, obj);

    let derive_only = obj.is_attr_set(ATTR_RUST_DERIVE_ONLY);
    let quoted_derive_clone_debug = if derive_only {
        quote!()
    } else {
        quote_derive_clone_debug()
    };
    let quoted_derive_clause = if derive_only {
        quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE_ONLY, "derive")
    } else {
        quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE, "derive")
    };
    let quoted_repr_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_REPR, "repr");
    let quoted_custom_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_CUSTOM_CLAUSE, "");

    let quoted_fields = fields
        .iter()
        .map(|obj_field| ObjectFieldTokenizer(reporter, obj, obj_field).quoted(objects));

    let quoted_deprecation_notice = if let Some(deprecation_notice) = obj.deprecation_notice() {
        quote!(#[deprecated(note = #deprecation_notice)])
    } else {
        quote!()
    };

    let is_tuple_struct = is_tuple_struct_from_obj(obj);
    let quoted_struct = if is_tuple_struct {
        quote! { pub struct #name(#(#quoted_fields,)*); }
    } else {
        quote! { pub struct #name { #(#quoted_fields,)* }}
    };

    let quoted_from_impl = quote_from_impl_from_obj(obj);

    let quoted_trait_impls = quote_trait_impls_from_obj(reporter, arrow_registry, objects, obj);

    let quoted_builder = quote_builder_from_obj(reporter, objects, obj);

    let quoted_heap_size_bytes = {
        let heap_size_bytes_impl = if is_tuple_struct_from_obj(obj) {
            quote!(self.0.heap_size_bytes())
        } else if obj.fields.is_empty() {
            quote!(0)
        } else {
            let quoted_heap_size_bytes = obj.fields.iter().map(|obj_field| {
                let field_name = format_ident!("{}", obj_field.name);
                quote!(self.#field_name.heap_size_bytes())
            });
            quote!(#(#quoted_heap_size_bytes)+*)
        };

        let is_pod_impl = if obj.fields.is_empty() {
            quote!(true)
        } else {
            let quoted_is_pods = obj.fields.iter().map(|obj_field| {
                let quoted_field_type = quote_field_type_from_object_field(obj_field);
                quote!(<#quoted_field_type>::is_pod())
            });
            quote!(#(#quoted_is_pods)&&*)
        };

        quote! {
            impl ::re_byte_size::SizeBytes for #name {
                #[inline]
                fn heap_size_bytes(&self) -> u64 {
                    #heap_size_bytes_impl
                }

                #[inline]
                fn is_pod() -> bool {
                    #is_pod_impl
                }
            }
        }
    };

    let tokens = quote! {
        #quoted_doc
        #quoted_derive_clone_debug
        #quoted_derive_clause
        #quoted_repr_clause
        #quoted_custom_clause
        #quoted_deprecation_notice
        #quoted_struct

        #quoted_trait_impls

        #quoted_from_impl

        #quoted_builder

        #quoted_heap_size_bytes
    };

    tokens
}

fn quote_union(
    reporter: &Reporter,
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    assert_eq!(obj.class, ObjectClass::Union);

    let Object { name, fields, .. } = obj;

    let name = format_ident!("{name}");

    let quoted_doc = quote_obj_docs(reporter, objects, obj);
    let derive_only = obj.try_get_attr::<String>(ATTR_RUST_DERIVE_ONLY).is_some();
    let quoted_derive_clone_debug = if derive_only {
        quote!()
    } else {
        quote_derive_clone_debug()
    };
    let quoted_derive_clause = if derive_only {
        quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE_ONLY, "derive")
    } else {
        quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE, "derive")
    };
    let quoted_repr_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_REPR, "repr");
    let quoted_custom_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_CUSTOM_CLAUSE, "");

    let quoted_fields = fields.iter().map(|obj_field| {
        let name = format_ident!("{}", re_case::to_pascal_case(&obj_field.name));

        let quoted_doc = quote_field_docs(reporter, objects, obj_field);
        let quoted_type = quote_field_type_from_object_field(obj_field);

        if obj_field.typ == Type::Unit {
            quote! {
                #quoted_doc
                #name
            }
        } else {
            quote! {
                #quoted_doc
                #name(#quoted_type)
            }
        }
    });

    let quoted_trait_impls = quote_trait_impls_from_obj(reporter, arrow_registry, objects, obj);

    let quoted_heap_size_bytes = {
        let quoted_matches = fields.iter().map(|obj_field| {
            let name = format_ident!("{}", re_case::to_pascal_case(&obj_field.name));

            if obj_field.typ == Type::Unit {
                quote!(Self::#name => 0)
            } else {
                quote!(Self::#name(v) => v.heap_size_bytes())
            }
        });

        let is_pod_impl = {
            let quoted_is_pods: Vec<_> = obj
                .fields
                .iter()
                .filter(|obj_field| obj_field.typ != Type::Unit)
                .map(|obj_field| {
                    let quoted_field_type = quote_field_type_from_object_field(obj_field);
                    quote!(<#quoted_field_type>::is_pod())
                })
                .collect();
            if quoted_is_pods.is_empty() {
                quote!(true)
            } else {
                quote!(#(#quoted_is_pods)&&*)
            }
        };

        quote! {
            impl ::re_byte_size::SizeBytes for #name {
                #[inline]
                fn heap_size_bytes(&self) -> u64 {
                    #![allow(clippy::match_same_arms)]
                    match self {
                        #(#quoted_matches),*
                    }
                }

                #[inline]
                fn is_pod() -> bool {
                    #is_pod_impl
                }
            }
        }
    };

    let tokens = quote! {
        #quoted_doc
        #quoted_derive_clone_debug
        #quoted_derive_clause
        #quoted_repr_clause
        #quoted_custom_clause
        pub enum #name {
            #(#quoted_fields,)*
        }

        #quoted_trait_impls

        #quoted_heap_size_bytes
    };

    tokens
}

// Pure C-style enum
fn quote_enum(
    reporter: &Reporter,
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    assert_eq!(obj.class, ObjectClass::Enum);

    let Object { name, fields, .. } = obj;

    let name = format_ident!("{name}");

    let quoted_doc = quote_obj_docs(reporter, objects, obj);
    let quoted_custom_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_CUSTOM_CLAUSE, "");

    let mut derives = vec!["Clone", "Copy", "Debug", "Hash", "PartialEq", "Eq"];

    match fields
        .iter()
        .filter(|field| field.attrs.has(ATTR_DEFAULT))
        .count()
    {
        0 => {}
        1 => {
            derives.push("Default");
        }
        _ => {
            reporter.error(
                &obj.virtpath,
                &obj.fqname,
                "Enums can only have one default value",
            );
        }
    };
    let derives = derives.iter().map(|&derive| {
        let derive = format_ident!("{derive}");
        quote!(#derive)
    });

    // NOTE: we keep the casing of the enum variants exactly as specified in the .fbs file,
    // or else `RGBA` would become `Rgba` and so on.
    // Note that we want consistency across:
    // * all languages (C++, Python, Rust)
    // * the arrow datatype
    // * the GUI

    let quoted_fields = fields.iter().map(|field| {
        let name = format_ident!("{}", field.name);

        if let Some(enum_value) = field.enum_value {
            let quoted_enum = proc_macro2::Literal::u8_unsuffixed(enum_value);
            let quoted_doc = quote_field_docs(reporter, objects, field);

            let default_attr = if field.attrs.has(ATTR_DEFAULT) {
                quote!(#[default])
            } else {
                quote!()
            };

            let clippy_attrs = if field.name == field.pascal_case_name() {
                quote!()
            } else {
                quote!(#[allow(clippy::upper_case_acronyms)]) // e.g. for `ColorModel::RGBA`
            };

            quote! {
                #quoted_doc
                #default_attr
                #clippy_attrs
                #name = #quoted_enum
            }
        } else {
            reporter.error(
                &field.virtpath,
                &field.fqname,
                "Enum ObjectFields must have an enum_value. This is likely a bug.",
            );
            quote! {}
        }
    });

    let quoted_trait_impls = quote_trait_impls_from_obj(reporter, arrow_registry, objects, obj);

    let all = fields.iter().map(|field| {
        let name = format_ident!("{}", field.name);
        quote!(Self::#name)
    });

    let display_match_arms = fields.iter().map(|field| {
        let name = &field.name;
        let quoted_name = format_ident!("{}", name);
        quote!(Self::#quoted_name => write!(f, #name))
    });
    let docstring_md_match_arms = fields.iter().map(|field| {
        let quoted_name = format_ident!("{}", field.name);
        let docstring_md = doc_as_lines(
            reporter,
            objects,
            &field.virtpath,
            &field.fqname,
            &field.docs,
            Target::WebDocsMarkdown,
            false,
        )
        .join("\n");
        if docstring_md.is_empty() {
            reporter.error(
                &field.virtpath,
                &field.fqname,
                "Missing documentation for enum variant. These are shown in the UI on hover.",
            );
        }
        quote!(Self::#quoted_name => #docstring_md)
    });

    let tokens = quote! {
        #quoted_doc
        #[derive( #(#derives,)* )]
        #quoted_custom_clause
        #[repr(u8)]
        pub enum #name {
            #(#quoted_fields,)*
        }

        #quoted_trait_impls

        // We implement `Display` to match the `PascalCase` name so that
        // the enum variants are displayed in the UI exactly how they are displayed in code.
        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #(#display_match_arms,)*
                }
            }
        }

        impl ::re_types_core::reflection::Enum for #name {

            #[inline]
            fn variants() -> &'static [Self] {
                &[#(#all),*]
            }

            #[inline]
            fn docstring_md(self) -> &'static str {
                match self {
                    #(#docstring_md_match_arms,)*
                }
            }
        }

        impl ::re_byte_size::SizeBytes for #name {
            #[inline]
            fn heap_size_bytes(&self) -> u64 {
                0
            }

            #[inline]
            fn is_pod() -> bool {
                true
            }
        }
    };

    tokens
}

// --- Code generators ---

struct ObjectFieldTokenizer<'a>(&'a Reporter, &'a Object, &'a ObjectField);

impl ObjectFieldTokenizer<'_> {
    fn quoted(&self, objects: &Objects) -> TokenStream {
        let Self(reporter, obj, obj_field) = self;
        let quoted_docs = quote_field_docs(reporter, objects, obj_field);
        let name = format_ident!("{}", &obj_field.name);
        let quoted_type = quote_field_type_from_object_field(obj_field);

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
    }
}

fn quote_field_docs(reporter: &Reporter, objects: &Objects, field: &ObjectField) -> TokenStream {
    let lines = doc_as_lines(
        reporter,
        objects,
        &field.virtpath,
        &field.fqname,
        &field.docs,
        Target::Rust,
        false,
    );

    let require_field_docs = false;
    if require_field_docs && lines.is_empty() && !field.is_testing() {
        reporter.warn(&field.virtpath, &field.fqname, "Missing documentation");
    }

    quote_doc_lines(&lines)
}

fn quote_obj_docs(reporter: &Reporter, objects: &Objects, obj: &Object) -> TokenStream {
    let mut lines = doc_as_lines(
        reporter,
        objects,
        &obj.virtpath,
        &obj.fqname,
        &obj.docs,
        Target::Rust,
        obj.is_experimental(),
    );

    // Prefix first line with `**Datatype**: ` etc:
    if let Some(first) = lines.first_mut() {
        *first = format!("**{}**: {}", obj.kind.singular_name(), first.trim());
    } else if !obj.is_testing() {
        reporter.error(&obj.virtpath, &obj.fqname, "Missing documentation for");
    }

    quote_doc_lines(&lines)
}

/// Returns type name as string and whether it was force unwrapped.
///
/// Specifying `unwrap = true` will unwrap the final type before returning it, e.g. `Vec<String>`
/// becomes just `String`.
/// The returned boolean indicates whether there was anything to unwrap at all.
fn quote_field_type_from_typ(typ: &Type, unwrap: bool) -> (TokenStream, bool) {
    let obj_field_type = TypeTokenizer { typ, unwrap };
    let unwrapped = unwrap && matches!(typ, Type::Array { .. } | Type::Vector { .. });
    (quote!(#obj_field_type), unwrapped)
}

fn quote_field_type_from_object_field(obj_field: &ObjectField) -> TokenStream {
    let (quoted_type, _) = quote_field_type_from_typ(&obj_field.typ, false);

    if obj_field.is_nullable {
        quote!(Option<#quoted_type>)
    } else {
        quoted_type
    }
}

struct TypeTokenizer<'a> {
    typ: &'a Type,
    unwrap: bool,
}

impl quote::ToTokens for TypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { typ, unwrap } = self;
        match typ {
            Type::Unit => quote!(()),
            Type::UInt8 => quote!(u8),
            Type::UInt16 => quote!(u16),
            Type::UInt32 => quote!(u32),
            Type::UInt64 => quote!(u64),
            Type::Int8 => quote!(i8),
            Type::Int16 => quote!(i16),
            Type::Int32 => quote!(i32),
            Type::Int64 => quote!(i64),
            Type::Bool => quote!(bool),
            Type::Float16 => quote!(half::f16),
            Type::Float32 => quote!(f32),
            Type::Float64 => quote!(f64),
            Type::String => quote!(::re_types_core::ArrowString),
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
                } else if elem_type.backed_by_arrow_buffer() {
                    quote!(::re_types_core::ArrowBuffer<#elem_type>)
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
            ElementType::Float16 => quote!(half::f16),
            ElementType::Float32 => quote!(f32),
            ElementType::Float64 => quote!(f64),
            ElementType::String => quote!(::re_types_core::ArrowString),
            ElementType::Object(fqname) => quote_fqname_as_type_path(fqname),
        }
        .to_tokens(tokens);
    }
}

fn quote_derive_clone_debug() -> TokenStream {
    quote!(#[derive(Clone, Debug)])
}

fn quote_meta_clause_from_obj(obj: &Object, attr: &str, clause: &str) -> TokenStream {
    let quoted = obj
        .try_get_attr::<String>(attr)
        .map(|contents| {
            if clause.is_empty() {
                syn::parse_str::<syn::Meta>(contents.as_str())
                    .with_context(|| format!("illegal meta clause: {clause:?}"))
                    .unwrap()
            } else {
                syn::parse_str::<syn::Meta>(&format!("{clause}({contents})"))
                    .with_context(|| format!("illegal meta clause: {clause}({contents})"))
                    .unwrap()
            }
        })
        .map(|clause| quote!(#[#clause]));
    quote!(#quoted)
}

fn quote_trait_impls_from_obj(
    reporter: &Reporter,
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    match obj.kind {
        ObjectKind::Datatype | ObjectKind::Component => {
            quote_trait_impls_for_datatype_or_component(objects, arrow_registry, obj)
        }

        ObjectKind::Archetype => quote_trait_impls_for_archetype(obj),

        ObjectKind::View => quote_trait_impls_for_view(reporter, obj),
    }
}

fn quote_trait_impls_for_datatype_or_component(
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
    obj: &Object,
) -> TokenStream {
    let Object {
        fqname, name, kind, ..
    } = obj;

    assert!(matches!(kind, ObjectKind::Datatype | ObjectKind::Component));

    let name = format_ident!("{name}");

    let datatype = arrow_registry.get(fqname);

    let optimize_for_buffer_slice = should_optimize_buffer_slice_deserialize(obj, arrow_registry);

    let is_forwarded_type = obj.is_arrow_transparent()
        && !obj.fields[0].is_nullable
        && matches!(obj.fields[0].typ, Type::Object(_));
    let forwarded_type =
        is_forwarded_type.then(|| quote_field_type_from_typ(&obj.fields[0].typ, true).0);

    let quoted_arrow_datatype = if let Some(forwarded_type) = forwarded_type.as_ref() {
        quote! {
            #[inline]
            fn arrow_datatype() -> arrow::datatypes::DataType {
                #forwarded_type::arrow_datatype()
            }
        }
    } else {
        let datatype = ArrowDataTypeTokenizer(&datatype, false);
        quote! {
            #[inline]
            fn arrow_datatype() -> arrow::datatypes::DataType {
                #![allow(clippy::wildcard_imports)]
                use arrow::datatypes::*;
                #datatype
            }
        }
    };

    let quoted_from_arrow2 = if optimize_for_buffer_slice {
        let from_arrow2_body = if let Some(forwarded_type) = forwarded_type.as_ref() {
            let is_pod = obj
                .try_get_attr::<String>(ATTR_RUST_DERIVE)
                .map_or(false, |d| d.contains("bytemuck::Pod"))
                || obj
                    .try_get_attr::<String>(ATTR_RUST_DERIVE_ONLY)
                    .map_or(false, |d| d.contains("bytemuck::Pod"));
            if is_pod {
                quote! {
                    #forwarded_type::from_arrow2(arrow_data).map(bytemuck::cast_vec)
                }
            } else {
                quote! {
                    #forwarded_type::from_arrow2(arrow_data).map(|v| v.into_iter().map(Self).collect())
                }
            }
        } else {
            let quoted_deserializer =
                quote_arrow_deserializer_buffer_slice(arrow_registry, objects, obj);

            quote! {
                // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
                // re_tracing::profile_function!();

                #![allow(clippy::wildcard_imports)]
                use arrow::datatypes::*;
                use arrow2::{ array::*, buffer::*};
                use ::re_types_core::{Loggable as _, ResultExt as _};

                // This code-path cannot have null fields. If it does have a validity mask
                // all bits must indicate valid data.
                if let Some(validity) = arrow_data.validity() {
                    if validity.unset_bits() != 0 {
                        return Err(DeserializationError::missing_data());
                    }
                }

                Ok(#quoted_deserializer)
            }
        };

        quote! {
            #[inline]
            fn from_arrow2(
                arrow_data: &dyn arrow2::array::Array,
            ) -> DeserializationResult<Vec<Self>>
            where
                Self: Sized
            {
                #from_arrow2_body
            }
        }
    } else {
        quote!()
    };

    // Forward deserialization to existing datatype if it's transparent.
    let quoted_deserializer = if let Some(forwarded_type) = forwarded_type.as_ref() {
        quote! {
            #forwarded_type::from_arrow2_opt(arrow_data).map(|v| v.into_iter().map(|v| v.map(Self)).collect())
        }
    } else {
        let quoted_deserializer = quote_arrow_deserializer(arrow_registry, objects, obj);
        quote! {
            // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
            // re_tracing::profile_function!();

            #![allow(clippy::wildcard_imports)]
            use arrow::datatypes::*;
            use arrow2::{ array::*, buffer::*};
            use ::re_types_core::{Loggable as _, ResultExt as _};
            Ok(#quoted_deserializer)
        }
    };

    let quoted_serializer = if let Some(forwarded_type) = forwarded_type.as_ref() {
        quote! {
            fn to_arrow_opt<'a>(
                data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
            ) -> SerializationResult<arrow::array::ArrayRef>
            where
                Self: Clone + 'a,
            {
                #forwarded_type::to_arrow_opt(data.into_iter().map(|datum| {
                    datum.map(|datum| match datum.into() {
                        ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                        ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
                    })
                }))
            }
        }
    } else {
        let quoted_serializer =
            quote_arrow_serializer(arrow_registry, objects, obj, &format_ident!("data"));

        quote! {
            // NOTE: Don't inline this, this gets _huge_.
            fn to_arrow_opt<'a>(
                data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
            ) -> SerializationResult<arrow::array::ArrayRef>
            where
                Self: Clone + 'a
            {
                // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
                // re_tracing::profile_function!();

                #![allow(clippy::wildcard_imports)]
                #![allow(clippy::manual_is_variant_and)]
                use arrow::{array::*, buffer::*, datatypes::*};
                use ::re_types_core::{Loggable as _, ResultExt as _, arrow_helpers::as_array_ref};

                Ok(#quoted_serializer)
            }
        }
    };

    let quoted_impl_component = (obj.kind == ObjectKind::Component).then(|| {
        quote! {
            impl ::re_types_core::Component for #name {
                #[inline]
                fn descriptor() -> ComponentDescriptor {
                    ComponentDescriptor::new(#fqname)
                }
            }
        }
    });

    quote! {
        #quoted_impl_component

        ::re_types_core::macros::impl_into_cow!(#name);

        impl ::re_types_core::Loggable for #name {
            #quoted_arrow_datatype

            #quoted_serializer

            // NOTE: Don't inline this, this gets _huge_.
            fn from_arrow2_opt(
                arrow_data: &dyn arrow2::array::Array,
            ) -> DeserializationResult<Vec<Option<Self>>>
            where
                Self: Sized
            {
                #quoted_deserializer
            }

            #quoted_from_arrow2
        }
    }
}

fn quote_trait_impls_for_archetype(obj: &Object) -> TokenStream {
    #![allow(clippy::collapsible_else_if)]

    let Object {
        fqname, name, kind, ..
    } = obj;

    assert_eq!(kind, &ObjectKind::Archetype);

    let display_name = re_case::to_human_case(name);
    let archetype_name = &obj.fqname;
    let name = format_ident!("{name}");

    fn compute_component_descriptors(
        obj: &Object,
        requirement_attr_value: &'static str,
    ) -> (usize, TokenStream) {
        let descriptors = obj
            .fields
            .iter()
            .filter_map(move |field| {
                field
                    .try_get_attr::<String>(requirement_attr_value)
                    .map(|_| {
                        let Some(component_name) = field.typ.fqname() else {
                            panic!("Archetype field must be an object/union or an array/vector of such")
                        };

                        let archetype_name = &obj.fqname;
                        let archetype_field_name = field.snake_case_name();

                        quote!(ComponentDescriptor {
                            archetype_name: Some(#archetype_name.into()),
                            component_name: #component_name.into(),
                            archetype_field_name: Some(#archetype_field_name.into()),
                        })
                    })
            })
            .collect_vec();

        let num_descriptors = descriptors.len();
        let quoted_descriptors = quote!(#(#descriptors,)*);

        (num_descriptors, quoted_descriptors)
    }

    let indicator_name = format!("{}Indicator", obj.name);

    let quoted_indicator_name = format_ident!("{indicator_name}");
    let quoted_indicator_doc =
        format!("Indicator component for the [`{name}`] [`::re_types_core::Archetype`]");
    let indicator_component_name =
        format!("{}Indicator", fqname.replace("archetypes", "components"));

    let (num_required_descriptors, required_descriptors) =
        compute_component_descriptors(obj, ATTR_RERUN_COMPONENT_REQUIRED);
    let (mut num_recommended_descriptors, mut recommended_descriptors) =
        compute_component_descriptors(obj, ATTR_RERUN_COMPONENT_RECOMMENDED);
    let (num_optional_descriptors, optional_descriptors) =
        compute_component_descriptors(obj, ATTR_RERUN_COMPONENT_OPTIONAL);

    num_recommended_descriptors += 1;
    recommended_descriptors = quote! {
        #recommended_descriptors
        ComponentDescriptor {
            archetype_name: Some(#archetype_name.into()),
            component_name: #indicator_component_name.into(),
            archetype_field_name: None,
        },
    };

    let num_components_docstring = quote_doc_line(&format!(
        "The total number of components in the archetype: {num_required_descriptors} required, {num_recommended_descriptors} recommended, {num_optional_descriptors} optional"
    ));
    let num_all_descriptors =
        num_required_descriptors + num_recommended_descriptors + num_optional_descriptors;

    let quoted_field_names = obj
        .fields
        .iter()
        .map(|field| format_ident!("{}", field.name))
        .collect::<Vec<_>>();

    let all_component_batches = {
        std::iter::once(quote! {
            Some(Self::indicator())
        }).chain(obj.fields.iter().map(|obj_field| {
            let field_name = format_ident!("{}", obj_field.name);
            let is_plural = obj_field.typ.is_plural();
            let is_nullable = obj_field.is_nullable;

            // NOTE: The nullability we're dealing with here is the nullability of an entire array of components,
            // not the nullability of individual elements (i.e. instances)!
            let batch = if is_nullable {
                if obj.attrs.has(ATTR_RERUN_LOG_MISSING_AS_EMPTY) {
                    if is_plural {
                        // Always log Option<Vec<C>> as Vec<V>, mapping None to empty batch
                        let component_type = quote_field_type_from_typ(&obj_field.typ, false).0;
                        quote! {
                            Some(
                                if let Some(comp_batch) = &self.#field_name {
                                    (comp_batch as &dyn ComponentBatch)
                                } else {
                                    // We need a reference to something that outives the function call
                                    static EMPTY_BATCH: once_cell::sync::OnceCell<#component_type> = once_cell::sync::OnceCell::new();
                                    let empty_batch: &#component_type = EMPTY_BATCH.get_or_init(|| Vec::new());
                                    (empty_batch as &dyn ComponentBatch)
                                }
                            )
                        }
                    } else {
                        // Always log Option<C>, mapping None to empty batch
                        quote!{ Some(&self.#field_name as &dyn ComponentBatch) }
                    }
                } else {
                    if is_plural {
                        // Maybe logging an Option<Vec<C>>
                        quote!{ self.#field_name.as_ref().map(|comp_batch| (comp_batch as &dyn ComponentBatch)) }
                    } else {
                        // Maybe logging an Option<C>
                        quote!{ self.#field_name.as_ref().map(|comp| (comp as &dyn ComponentBatch)) }
                    }
                }
            } else {
                // Always logging a Vec<C> or C
                quote!{ Some(&self.#field_name as &dyn ComponentBatch) }
            };

            let Some(component_name) = obj_field.typ.fqname() else {
                panic!("Archetype field must be an object/union or an array/vector of such")
            };
            let archetype_name = &obj.fqname;
            let archetype_field_name = obj_field.snake_case_name();

            quote! {
                (#batch).map(|batch| {
                    ::re_types_core::ComponentBatchCowWithDescriptor {
                        batch: batch.into(),
                        descriptor_override: Some(ComponentDescriptor {
                            archetype_name: Some(#archetype_name.into()),
                            archetype_field_name: Some((#archetype_field_name).into()),
                            component_name: (#component_name).into(),
                        }),
                    }
                })

            }
        }))
    };

    let all_deserializers = {
        obj.fields.iter().map(|obj_field| {
            let obj_field_fqname = obj_field.fqname.as_str();
            let field_typ_fqname_str = obj_field.typ.fqname().unwrap();
            let field_name = format_ident!("{}", obj_field.name);

            let is_plural = obj_field.typ.is_plural();
            let is_nullable = obj_field.is_nullable;

            // NOTE: unwrapping is safe since the field must point to a component.
            let component = quote_fqname_as_type_path(obj_field.typ.fqname().unwrap());

            let quoted_collection = if is_plural {
                quote! {
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context(#obj_field_fqname)?
                }
            } else {
                quote! {
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(DeserializationError::missing_data)
                    .with_context(#obj_field_fqname)?
                }
            };


            // NOTE: An archetype cannot have overlapped component types by definition, so use the
            // component's fqname to do the mapping.
            let quoted_deser = if is_nullable && !is_plural {
                // For a nullable mono-component, it's valid for data to be missing
                // after a clear.
                let quoted_collection =
                    quote! {
                        .into_iter()
                        .next()
                        .flatten()
                    };

                quote! {
                    if let Some(array) = arrays_by_name.get(#field_typ_fqname_str) {
                        <#component>::from_arrow2_opt(&**array)
                            .with_context(#obj_field_fqname)?
                            #quoted_collection
                    } else {
                        None
                    }
                }
            } else if is_nullable {
                quote! {
                    if let Some(array) = arrays_by_name.get(#field_typ_fqname_str) {
                        Some({
                            <#component>::from_arrow2_opt(&**array)
                                .with_context(#obj_field_fqname)?
                                #quoted_collection
                        })
                    } else {
                        None
                    }
                }
            } else {
                quote! {{
                    let array = arrays_by_name
                        .get(#field_typ_fqname_str)
                        .ok_or_else(DeserializationError::missing_data)
                        .with_context(#obj_field_fqname)?;

                    <#component>::from_arrow2_opt(&**array).with_context(#obj_field_fqname)? #quoted_collection
                }}
            };

            quote!(let #field_name = #quoted_deser;)
        })
    };

    quote! {
        static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; #num_required_descriptors]> =
            once_cell::sync::Lazy::new(|| {[#required_descriptors]});

        static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; #num_recommended_descriptors]> =
            once_cell::sync::Lazy::new(|| {[#recommended_descriptors]});

        static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; #num_optional_descriptors]> =
            once_cell::sync::Lazy::new(|| {[#optional_descriptors]});

        static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; #num_all_descriptors]> =
            once_cell::sync::Lazy::new(|| {[#required_descriptors #recommended_descriptors #optional_descriptors]});

        impl #name {
            #num_components_docstring
            pub const NUM_COMPONENTS: usize = #num_all_descriptors;
        }

        #[doc = #quoted_indicator_doc]
        pub type #quoted_indicator_name = ::re_types_core::GenericIndicatorComponent<#name>;

        impl ::re_types_core::Archetype for #name {
            type Indicator = #quoted_indicator_name;

            #[inline]
            fn name() -> ::re_types_core::ArchetypeName {
                #fqname.into()
            }

            #[inline]
            fn display_name() -> &'static str {
                #display_name
            }

            #[inline]
            fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
                static INDICATOR: #quoted_indicator_name = #quoted_indicator_name::DEFAULT;
                ComponentBatchCowWithDescriptor::new(&INDICATOR as &dyn ::re_types_core::ComponentBatch)
            }

            #[inline]
            fn required_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
                REQUIRED_COMPONENTS.as_slice().into()
            }

            #[inline]
            fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]>  {
                RECOMMENDED_COMPONENTS.as_slice().into()
            }

            #[inline]
            fn optional_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]>  {
                OPTIONAL_COMPONENTS.as_slice().into()
            }

            // NOTE: Don't rely on default implementation so that we can keep everything static.
            #[inline]
            fn all_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]>  {
                ALL_COMPONENTS.as_slice().into()
            }

            #[inline]
            fn from_arrow2_components(
                arrow_data: impl IntoIterator<Item = (
                    ComponentName,
                    Box<dyn arrow2::array::Array>,
                )>,
            ) -> DeserializationResult<Self> {
                re_tracing::profile_function!();

                use ::re_types_core::{Loggable as _, ResultExt as _};

                // NOTE: Even though ComponentName is an InternedString, we must
                // convert to &str here because the .get("component.name") accessors
                // will fail otherwise.
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
                    .into_iter()
                    .map(|(name, array)| (name.full_name(), array)).collect();

                #(#all_deserializers;)*

                Ok(Self {
                    #(#quoted_field_names,)*
                })
            }
        }

        impl ::re_types_core::AsComponents for #name {
            fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
                re_tracing::profile_function!();

                use ::re_types_core::Archetype as _;
                [#(#all_component_batches,)*].into_iter().flatten().collect()
            }
        }

        impl ::re_types_core::ArchetypeReflectionMarker for #name { }
    }
}

fn quote_trait_impls_for_view(reporter: &Reporter, obj: &Object) -> TokenStream {
    assert_eq!(obj.kind, ObjectKind::View);

    let name = format_ident!("{}", obj.name);

    let Some(identifier): Option<String> = obj.try_get_attr(ATTR_RERUN_VIEW_IDENTIFIER) else {
        reporter.error(
            &obj.virtpath,
            &obj.fqname,
            format!("Missing {ATTR_RERUN_VIEW_IDENTIFIER} attribute for view"),
        );
        return TokenStream::new();
    };

    quote! {
        impl ::re_types_core::View for #name {
            #[inline]
            fn identifier() -> ::re_types_core::ViewClassIdentifier {
                #identifier .into()
            }
        }
    }
}

/// Only makes sense for components & datatypes.
fn quote_from_impl_from_obj(obj: &Object) -> TokenStream {
    if obj.kind == ObjectKind::Archetype {
        return TokenStream::new();
    }
    if obj.fields.len() != 1 {
        return TokenStream::new();
    }

    let obj_is_tuple_struct = is_tuple_struct_from_obj(obj);
    let obj_field = &obj.fields[0];
    let quoted_obj_name = format_ident!("{}", obj.name);
    let quoted_obj_field_name = format_ident!("{}", obj_field.name);

    let quoted_type = quote_field_type_from_object_field(obj_field);

    let self_field_access = if obj_is_tuple_struct {
        quote!(self.0)
    } else {
        quote!(self.#quoted_obj_field_name )
    };
    let deref_impl = quote! {
        impl std::ops::Deref for #quoted_obj_name {
            type Target = #quoted_type;

            #[inline]
            fn deref(&self) -> &#quoted_type {
                &#self_field_access
            }
        }

        impl std::ops::DerefMut for #quoted_obj_name {
            #[inline]
            fn deref_mut(&mut self) -> &mut #quoted_type {
                &mut #self_field_access
            }
        }
    };

    if obj_field.typ.fqname().is_some() {
        if let Some(inner) = obj_field.typ.vector_inner() {
            if obj_field.is_nullable {
                let quoted_binding = if obj_is_tuple_struct {
                    quote!(Self(v.map(|v| v.into_iter().map(|v| v.into()).collect())))
                } else {
                    quote!(Self { #quoted_obj_field_name: v.map(|v| v.into_iter().map(|v| v.into()).collect()) })
                };

                quote! {
                    impl<I: Into<#inner>, T: IntoIterator<Item = I>> From<Option<T>> for #quoted_obj_name {
                        fn from(v: Option<T>) -> Self {
                            #quoted_binding
                        }
                    }
                }
            } else {
                let quoted_binding = if obj_is_tuple_struct {
                    quote!(Self(v.into_iter().map(|v| v.into()).collect()))
                } else {
                    quote!(Self { #quoted_obj_field_name: v.into_iter().map(|v| v.into()).collect() })
                };

                quote! {
                    impl<I: Into<#inner>, T: IntoIterator<Item = I>> From<T> for #quoted_obj_name {
                        fn from(v: T) -> Self {
                            #quoted_binding
                        }
                    }
                }
            }
        } else {
            let quoted_binding = if obj_is_tuple_struct {
                quote!(Self(v.into()))
            } else {
                quote!(Self { #quoted_obj_field_name: v.into() })
            };

            quote! {
                impl<T: Into<#quoted_type>> From<T> for #quoted_obj_name {
                    fn from(v: T) -> Self {
                        #quoted_binding
                    }
                }

                impl std::borrow::Borrow<#quoted_type> for #quoted_obj_name {
                    #[inline]
                    fn borrow(&self) -> &#quoted_type {
                        &#self_field_access
                    }
                }

                #deref_impl
            }
        }
    } else {
        let (quoted_binding, quoted_read) = if obj_is_tuple_struct {
            (quote!(Self(#quoted_obj_field_name)), quote!(value.0))
        } else {
            (
                quote!(Self { #quoted_obj_field_name }),
                quote!(value.#quoted_obj_field_name),
            )
        };

        // If the field is not a custom datatype, emit `Deref`/`DerefMut` only for components.
        // (in the long run all components are implemented with custom data types, making it so that we don't hit this path anymore)
        // For ObjectKind::Datatype we sometimes have custom implementations for `Deref`, e.g. `Utf8String` derefs to `&str` instead of `ArrowString`.
        let deref_impl = if obj.kind == ObjectKind::Component {
            deref_impl
        } else {
            quote!()
        };

        quote! {
            impl From<#quoted_type> for #quoted_obj_name {
                #[inline]
                fn from(#quoted_obj_field_name: #quoted_type) -> Self {
                    #quoted_binding
                }
            }

            impl From<#quoted_obj_name> for #quoted_type {
                #[inline]
                fn from(value: #quoted_obj_name) -> Self {
                    #quoted_read
                }
            }

            #deref_impl
        }
    }
}

/// Only makes sense for archetypes.
fn quote_builder_from_obj(reporter: &Reporter, objects: &Objects, obj: &Object) -> TokenStream {
    if obj.kind != ObjectKind::Archetype {
        return TokenStream::new();
    }

    let Object { name, fields, .. } = obj;

    let name = format_ident!("{name}");

    // NOTE: Collecting because we need to iterate them more than once.
    let required = fields
        .iter()
        .filter(|field| !field.is_nullable)
        .collect::<Vec<_>>();
    let optional = fields
        .iter()
        .filter(|field| field.is_nullable)
        .collect::<Vec<_>>();

    let fn_new = {
        // fn new()
        let quoted_params = required.iter().map(|field| {
            let field_name = format_ident!("{}", field.name);
            let (typ, unwrapped) = quote_field_type_from_typ(&field.typ, true);
            if unwrapped {
                // This was originally a vec/array!
                quote!(#field_name: impl IntoIterator<Item = impl Into<#typ>>)
            } else {
                quote!(#field_name: impl Into<#typ>)
            }
        });

        let quoted_required = required.iter().map(|field| {
            let field_name = format_ident!("{}", field.name);
            let (_, unwrapped) = quote_field_type_from_typ(&field.typ, true);
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

        let fn_new_pub = if obj.is_attr_set(ATTR_RUST_NEW_PUB_CRATE) {
            quote!(pub(crate))
        } else {
            quote!(pub)
        };

        if required.is_empty() && obj.attrs.has(ATTR_RERUN_LOG_MISSING_AS_EMPTY) {
            let docstring = quote_doc_line(&format!(
                "Create a new `{name}` which when logged will clear the values of all components."
            ));

            quote! {
                #docstring
                #[inline]
                #fn_new_pub fn clear() -> Self {
                    Self {
                        #(#quoted_optional,)*
                    }
                }
            }
        } else {
            let docstring = quote_doc_line(&format!("Create a new `{name}`."));

            quote! {
                #docstring
                #[inline]
                #fn_new_pub fn new(#(#quoted_params,)*) -> Self {
                    Self {
                        #(#quoted_required,)*
                        #(#quoted_optional,)*
                    }
                }
            }
        }
    };

    let with_methods = optional.iter().map(|field| {
        // fn with_*()
        let field_name = format_ident!("{}", field.name);
        let method_name = format_ident!("with_{field_name}");
        let (typ, unwrapped) = quote_field_type_from_typ(&field.typ, true);
        let docstring = quote_field_docs(reporter, objects, field);

        if unwrapped {
            // This was originally a vec/array!
            quote! {
                #docstring
                #[inline]
                pub fn #method_name(mut self, #field_name: impl IntoIterator<Item = impl Into<#typ>>) -> Self {
                    self.#field_name = Some(#field_name.into_iter().map(Into::into).collect());
                    self
                }
            }
        } else {
            quote! {
                #docstring
                #[inline]
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
