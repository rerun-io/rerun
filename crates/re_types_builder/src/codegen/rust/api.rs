use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools as _;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{
    codegen::{
        autogen_warning,
        common::{collect_examples_for_api_docs, ExampleInfo},
        rust::{
            arrow::ArrowDataTypeTokenizer,
            deserializer::{
                quote_arrow_deserializer, quote_arrow_deserializer_buffer_slice,
                should_optimize_buffer_slice_deserialize,
            },
            serializer::quote_arrow_serializer,
            util::{is_tuple_struct_from_obj, iter_archetype_components},
        },
        StringExt as _,
    },
    format_path,
    objects::ObjectType,
    ArrowRegistry, CodeGenerator, Docs, ElementType, Object, ObjectField, ObjectKind, Objects,
    Reporter, Type, ATTR_RERUN_COMPONENT_OPTIONAL, ATTR_RERUN_COMPONENT_RECOMMENDED,
    ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RUST_CUSTOM_CLAUSE, ATTR_RUST_DERIVE,
    ATTR_RUST_DERIVE_ONLY, ATTR_RUST_NEW_PUB_CRATE, ATTR_RUST_REPR,
};

use super::{
    arrow::quote_fqname_as_type_path, blueprint_validation::generate_blueprint_validation,
    util::string_from_quoted,
};

// ---

// TODO(cmc): it'd be nice to be able to generate vanilla comments (as opposed to doc-comments)
// once again at some point (`TokenStream` strips them)… nothing too urgent though.

// ---

type Result<T, E = anyhow::Error> = std::result::Result<T, E>;

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

        for object_kind in ObjectKind::ALL {
            self.generate_folder(
                reporter,
                objects,
                arrow_registry,
                object_kind,
                &mut files_to_write,
            );
        }

        generate_blueprint_validation(reporter, objects, &mut files_to_write);

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
    ) {
        let crates_root_path = self.workspace_path.join("crates");

        let mut all_modules: HashSet<_> = HashSet::default();

        // Generate folder contents:
        let ordered_objects = objects.ordered_objects(object_kind.into());
        for &obj in &ordered_objects {
            let crate_name = obj.crate_name();
            let module_name = obj.module_name();

            let crate_path = crates_root_path.join(&crate_name);
            let module_path = if obj.is_testing() {
                crate_path.join("src/testing").join(&module_name)
            } else {
                crate_path.join("src").join(&module_name)
            };

            let filename_stem = obj.snake_case_name();
            let filename = format!("{filename_stem}.rs");

            let filepath = module_path.join(filename);

            let mut code = match generate_object_file(reporter, objects, arrow_registry, obj) {
                Ok(code) => code,
                Err(err) => {
                    reporter.error(&obj.virtpath, &obj.fqname, err);
                    continue;
                }
            };

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
            let relevant_objs = &ordered_objects
                .iter()
                .filter(|obj| obj.is_testing() == is_testing)
                .filter(|obj| obj.crate_name() == crate_name)
                .filter(|obj| obj.module_name() == module_name)
                .copied()
                .collect_vec();

            // src/{testing/}{datatypes|components|archetypes}/mod.rs
            generate_mod_file(&module_path, relevant_objs, files_to_write);
        }
    }
}

fn generate_object_file(
    reporter: &Reporter,
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
    obj: &Object,
) -> Result<String> {
    let mut code = String::new();
    code.push_str(&format!("// {}\n", autogen_warning!()));
    if let Some(source_path) = obj.relative_filepath() {
        code.push_str(&format!("// Based on {:?}.\n\n", format_path(source_path)));
    }

    code.push_str("#![allow(trivial_numeric_casts)]\n");
    code.push_str("#![allow(unused_imports)]\n");
    code.push_str("#![allow(unused_parens)]\n");
    code.push_str("#![allow(clippy::clone_on_copy)]\n");
    code.push_str("#![allow(clippy::iter_on_single_items)]\n");
    code.push_str("#![allow(clippy::map_flatten)]\n");
    code.push_str("#![allow(clippy::match_wildcard_for_single_variants)]\n");
    code.push_str("#![allow(clippy::needless_question_mark)]\n");
    code.push_str("#![allow(clippy::new_without_default)]\n");
    code.push_str("#![allow(clippy::redundant_closure)]\n");
    code.push_str("#![allow(clippy::too_many_arguments)]\n");
    code.push_str("#![allow(clippy::too_many_lines)]\n");
    code.push_str("#![allow(clippy::unnecessary_cast)]\n");

    code.push_str("\n\n");

    code.push_str("use ::re_types_core::external::arrow2;\n");
    code.push_str("use ::re_types_core::SerializationResult;\n");
    code.push_str("use ::re_types_core::{DeserializationResult, DeserializationError};\n");
    code.push_str("use ::re_types_core::ComponentName;\n");
    code.push_str("use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};\n");

    let mut acc = TokenStream::new();

    // NOTE: `TokenStream`s discard whitespacing information by definition, so we need to
    // inject some of our own when writing to file… while making sure that don't inject
    // random spacing into doc comments that look like code!

    let quoted_obj = match obj.typ() {
        crate::objects::ObjectType::Struct => quote_struct(reporter, arrow_registry, objects, obj),
        crate::objects::ObjectType::Union => quote_union(reporter, arrow_registry, objects, obj),
        crate::objects::ObjectType::Enum => anyhow::bail!("Enums are not implemented in Rust"),
    };

    let mut tokens = quoted_obj.into_iter();
    while let Some(token) = tokens.next() {
        match &token {
            // If this is a doc-comment block, be smart about it.
            proc_macro2::TokenTree::Punct(punct) if punct.as_char() == '#' => {
                code.push_text(string_from_quoted(&acc), 1, 0);
                acc = TokenStream::new();

                acc.extend([token, tokens.next().unwrap()]);
                code.push_text(acc.to_string(), 1, 0);
                acc = TokenStream::new();
            }
            _ => {
                acc.extend([token]);
            }
        }
    }

    code.push_text(string_from_quoted(&acc), 1, 0);

    Ok(replace_doc_attrb_with_doc_comment(&code))
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

    code += "\n\n";

    for obj in objects {
        let module_name = obj.snake_case_name();
        let type_name = &obj.name;
        code.push_str(&format!("pub use self::{module_name}::{type_name};\n"));
    }

    files_to_write.insert(path, code);
}

/// Replace `#[doc = "…"]` attributes with `/// …` doc comments,
/// while also removing trailing whitespace.
fn replace_doc_attrb_with_doc_comment(code: &String) -> String {
    // This is difficult to do with regex, because the patterns with newlines overlap.

    let start_pattern = "# [doc = \"";
    let end_pattern = "\"]\n"; // assues there is no escaped quote followed by a bracket

    let problematic = r#"\"]\n"#;
    assert!(
        !code.contains(problematic),
        "The codegen cannot handle the string {problematic} yet"
    );

    let mut new_code = String::new();

    let mut i = 0;
    while i < code.len() {
        if let Some(off) = code[i..].find(start_pattern) {
            let doc_start = i + off;
            let content_start = doc_start + start_pattern.len();
            if let Some(off) = code[content_start..].find(end_pattern) {
                let content_end = content_start + off;
                new_code.push_str(&code[i..doc_start]);
                new_code.push_str("///");
                let content = &code[content_start..content_end];
                if !content.starts_with(char::is_whitespace) {
                    new_code.push(' ');
                }
                unescape_string_into(content, &mut new_code);
                new_code.push('\n');

                i = content_end + end_pattern.len();
                // Skip trailing whitespace (extra newlines)
                while matches!(code.as_bytes().get(i), Some(b'\n' | b' ')) {
                    i += 1;
                }
                continue;
            }
        }

        // No more doc attributes found
        new_code.push_str(&code[i..]);
        break;
    }
    new_code
}

fn unescape_string_into(input: &str, output: &mut String) {
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            let c = chars.next().expect("Trailing backslash");
            match c {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '\\' => output.push('\\'),
                '"' => output.push('"'),
                '\'' => output.push('\''),
                _ => panic!("Unknown escape sequence: \\{c}"),
            }
        } else {
            output.push(c);
        }
    }
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

    let quoted_doc = quote_obj_docs(reporter, obj);

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
        .map(|obj_field| ObjectFieldTokenizer(reporter, obj, obj_field));

    let is_tuple_struct = is_tuple_struct_from_obj(obj);
    let quoted_struct = if is_tuple_struct {
        quote! { pub struct #name(#(#quoted_fields,)*); }
    } else {
        quote! { pub struct #name { #(#quoted_fields,)* }}
    };

    let quoted_from_impl = quote_from_impl_from_obj(obj);

    let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, objects, obj);

    let quoted_builder = quote_builder_from_obj(obj);

    let quoted_heap_size_bytes = if obj
        .fields
        .iter()
        .any(|field| field.has_attr(crate::ATTR_RUST_SERDE_TYPE))
    {
        // TODO(cmc): serde types are a temporary hack that's not worth worrying about.
        quote!()
    } else {
        let heap_size_bytes_impl = if is_tuple_struct_from_obj(obj) {
            quote!(self.0.heap_size_bytes())
        } else {
            let quoted_heap_size_bytes = obj.fields.iter().map(|obj_field| {
                let field_name = format_ident!("{}", obj_field.name);
                quote!(self.#field_name.heap_size_bytes())
            });
            quote!(#(#quoted_heap_size_bytes)+*)
        };

        let is_pod_impl = {
            let quoted_is_pods = obj.fields.iter().map(|obj_field| {
                let quoted_field_type = quote_field_type_from_object_field(obj_field);
                quote!(<#quoted_field_type>::is_pod())
            });
            quote!(#(#quoted_is_pods)&&*)
        };

        quote! {
            impl ::re_types_core::SizeBytes for #name {
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
        #quoted_struct

        #quoted_heap_size_bytes

        #quoted_from_impl

        #quoted_trait_impls

        #quoted_builder
    };

    tokens
}

fn quote_union(
    reporter: &Reporter,
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    assert_eq!(obj.typ(), ObjectType::Union);

    let Object { name, fields, .. } = obj;

    let name = format_ident!("{name}");

    let quoted_doc = quote_obj_docs(reporter, obj);
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
        let name = format_ident!("{}", crate::to_pascal_case(&obj_field.name));

        let quoted_doc = quote_field_docs(reporter, obj_field);
        let quoted_type = quote_field_type_from_object_field(obj_field);

        quote! {
            #quoted_doc
            #name(#quoted_type)
        }
    });

    let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, objects, obj);

    let quoted_heap_size_bytes = if obj
        .fields
        .iter()
        .any(|field| field.has_attr(crate::ATTR_RUST_SERDE_TYPE))
    {
        // TODO(cmc): serde types are a temporary hack that's not worth worrying about.
        quote!()
    } else {
        let quoted_matches = fields.iter().map(|obj_field| {
            let name = format_ident!("{}", crate::to_pascal_case(&obj_field.name));
            quote!(Self::#name(v) => v.heap_size_bytes())
        });

        let is_pod_impl = {
            let quoted_is_pods = obj.fields.iter().map(|obj_field| {
                let quoted_field_type = quote_field_type_from_object_field(obj_field);
                quote!(<#quoted_field_type>::is_pod())
            });
            quote!(#(#quoted_is_pods)&&*)
        };

        quote! {
            impl ::re_types_core::SizeBytes for #name {
                #[allow(clippy::match_same_arms)]
                #[inline]
                fn heap_size_bytes(&self) -> u64 {
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

        #quoted_heap_size_bytes

        #quoted_trait_impls
    };

    tokens
}

// --- Code generators ---

struct ObjectFieldTokenizer<'a>(&'a Reporter, &'a Object, &'a ObjectField);

impl quote::ToTokens for ObjectFieldTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self(reporter, obj, obj_field) = self;
        let quoted_docs = quote_field_docs(reporter, obj_field);
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
        .to_tokens(tokens);
    }
}

fn quote_field_docs(reporter: &Reporter, field: &ObjectField) -> TokenStream {
    let require_example = false;
    let lines = doc_as_lines(
        reporter,
        &field.virtpath,
        &field.fqname,
        &field.docs,
        require_example,
    );

    let require_field_docs = false;
    if require_field_docs && lines.is_empty() && !field.is_testing() {
        reporter.warn(&field.virtpath, &field.fqname, "Missing documentation");
    }

    quote_doc_lines(&lines)
}

fn quote_obj_docs(reporter: &Reporter, obj: &Object) -> TokenStream {
    let require_example = obj.kind == ObjectKind::Archetype;
    let mut lines = doc_as_lines(
        reporter,
        &obj.virtpath,
        &obj.fqname,
        &obj.docs,
        require_example,
    );

    // Prefix first line with `**Datatype**: ` etc:
    if let Some(first) = lines.first_mut() {
        *first = format!("**{}**: {}", obj.kind.singular_name(), first.trim());
    } else if !obj.is_testing() {
        reporter.error(&obj.virtpath, &obj.fqname, "Missing documentation for");
    }

    quote_doc_lines(&lines)
}

fn doc_as_lines(
    reporter: &Reporter,
    virtpath: &str,
    fqname: &str,
    docs: &Docs,
    require_example: bool,
) -> Vec<String> {
    let mut lines = crate::codegen::get_documentation(docs, &["rs", "rust"]);

    let examples = collect_examples_for_api_docs(docs, "rs", true)
        .map_err(|err| reporter.error(virtpath, fqname, err))
        .unwrap_or_default();

    if examples.is_empty() {
        if require_example {
            reporter.warn(virtpath, fqname, "Missing example");
        }
    } else {
        lines.push(Default::default());
        let section_title = if examples.len() == 1 {
            "Example"
        } else {
            "Examples"
        };
        lines.push(format!("## {section_title}"));
        lines.push(Default::default());
        let mut examples = examples.into_iter().peekable();
        while let Some(example) = examples.next() {
            let ExampleInfo {
                name, title, image, ..
            } = &example.base;

            for line in &example.lines {
                if line.contains("```") {
                    reporter.error(
                        virtpath,
                        fqname,
                        format!("Example {name:?} contains ``` in it, so we can't embed it in the Rust API docs."),
                    );
                    continue;
                }
            }

            if let Some(title) = title {
                lines.push(format!("### {title}"));
            } else {
                lines.push(format!("### `{name}`:"));
            }

            lines.push("```ignore".into());
            lines.extend(example.lines.into_iter());
            lines.push("```".into());

            if let Some(image) = &image {
                lines.extend(image.image_stack().into_iter());
            }
            if examples.peek().is_some() {
                // blank line between examples
                lines.push(Default::default());
            }
        }
    }

    if let Some(second_line) = lines.get(1) {
        if !second_line.is_empty() {
            reporter.warn(
                virtpath,
                fqname,
                format!(
                    "Second line of documentation should be an empty line; found {second_line:?}"
                ),
            );
        }
    }

    lines
}

fn quote_doc_lines(lines: &[String]) -> TokenStream {
    struct DocCommentTokenizer<'a>(&'a [String]);

    impl quote::ToTokens for DocCommentTokenizer<'_> {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            tokens.extend(self.0.iter().map(|line| {
                let line = format!(" {line}"); // add space between `///` and comment
                quote!(# [doc = #line])
            }));
        }
    }

    let lines = DocCommentTokenizer(lines);
    quote!(#lines)
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
    let serde_type = obj_field.try_get_attr::<String>(crate::ATTR_RUST_SERDE_TYPE);
    let quoted_type = if let Some(serde_type) = serde_type {
        assert_eq!(
            &obj_field.typ,
            &Type::Vector {
                elem_type: ElementType::UInt8
            },
            "`attr.rust.serde_type` may only be used on fields of type `[ubyte]`",
        );

        let quoted_serde_type: syn::TypePath = syn::parse_str(&serde_type).unwrap();
        quote!(#quoted_serde_type)
    } else {
        quote_field_type_from_typ(&obj_field.typ, false).0
    };
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
            Type::UInt8 => quote!(u8),
            Type::UInt16 => quote!(u16),
            Type::UInt32 => quote!(u32),
            Type::UInt64 => quote!(u64),
            Type::Int8 => quote!(i8),
            Type::Int16 => quote!(i16),
            Type::Int32 => quote!(i32),
            Type::Int64 => quote!(i64),
            Type::Bool => quote!(bool),
            Type::Float16 => quote!(arrow2::types::f16),
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
            ElementType::Float16 => quote!(arrow2::types::f16),
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
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
    let Object {
        fqname, name, kind, ..
    } = obj;

    let name = format_ident!("{name}");

    match kind {
        ObjectKind::Datatype | ObjectKind::Component => {
            let quoted_kind = if *kind == ObjectKind::Datatype {
                quote!(Datatype)
            } else {
                quote!(Component)
            };
            let kind_name = format_ident!("{quoted_kind}Name");

            let datatype = arrow_registry.get(fqname);

            let optimize_for_buffer_slice =
                should_optimize_buffer_slice_deserialize(obj, arrow_registry);

            let datatype = ArrowDataTypeTokenizer(&datatype, false);

            let quoted_serializer =
                quote_arrow_serializer(arrow_registry, objects, obj, &format_ident!("data"));
            let quoted_deserializer = quote_arrow_deserializer(arrow_registry, objects, obj);

            let quoted_from_arrow = if optimize_for_buffer_slice {
                let quoted_deserializer =
                    quote_arrow_deserializer_buffer_slice(arrow_registry, objects, obj);

                quote! {
                    #[allow(clippy::wildcard_imports)]
                    #[inline]
                    fn from_arrow(
                        arrow_data: &dyn arrow2::array::Array,
                    ) -> DeserializationResult<Vec<Self>>
                    where
                        Self: Sized
                    {
                        // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
                        // re_tracing::profile_function!();

                        use arrow2::{datatypes::*, array::*, buffer::*};
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
                }
            } else {
                quote!()
            };

            quote! {
                ::re_types_core::macros::impl_into_cow!(#name);

                impl ::re_types_core::Loggable for #name {
                    type Name = ::re_types_core::#kind_name;

                    #[inline]
                    fn name() -> Self::Name {
                        #fqname.into()
                    }

                    #[allow(clippy::wildcard_imports)]
                    #[inline]
                    fn arrow_datatype() -> arrow2::datatypes::DataType {
                        use arrow2::datatypes::*;
                        #datatype
                    }

                    // NOTE: Don't inline this, this gets _huge_.
                    #[allow(clippy::wildcard_imports)]
                    fn to_arrow_opt<'a>(
                        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
                    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
                    where
                        Self: Clone + 'a
                    {
                        // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
                        // re_tracing::profile_function!();

                        use arrow2::{datatypes::*, array::*};
                        use ::re_types_core::{Loggable as _, ResultExt as _};

                        Ok(#quoted_serializer)
                    }

                    // NOTE: Don't inline this, this gets _huge_.
                    #[allow(clippy::wildcard_imports)]
                    fn from_arrow_opt(
                        arrow_data: &dyn arrow2::array::Array,
                    ) -> DeserializationResult<Vec<Option<Self>>>
                    where
                        Self: Sized
                    {
                        // NOTE(#3850): Don't add a profile scope here: the profiler overhead is too big for this fast function.
                        // re_tracing::profile_function!();

                        use arrow2::{datatypes::*, array::*, buffer::*};
                        use ::re_types_core::{Loggable as _, ResultExt as _};

                        Ok(#quoted_deserializer)
                    }

                    #quoted_from_arrow
                }
            }
        }

        ObjectKind::Archetype => {
            fn compute_components(
                obj: &Object,
                attr: &'static str,
                extras: impl IntoIterator<Item = String>,
            ) -> (usize, TokenStream) {
                let components = iter_archetype_components(obj, attr)
                    .chain(extras)
                    .collect::<BTreeSet<_>>();

                let num_components = components.len();
                let quoted_components = quote!(#(#components.into(),)*);

                (num_components, quoted_components)
            }

            let first_required_comp = obj.fields.iter().find(|field| {
                field
                    .try_get_attr::<String>(ATTR_RERUN_COMPONENT_REQUIRED)
                    .is_some()
            });

            let num_instances = if let Some(comp) = first_required_comp {
                if comp.typ.is_plural() {
                    let name = format_ident!("{}", comp.name);
                    quote!(self.#name.len())
                } else {
                    quote!(1)
                }
            } else {
                quote!(0)
            };

            let indicator_name = format!("{}Indicator", obj.name);
            let indicator_fqname =
                format!("{}Indicator", obj.fqname).replace("archetypes", "components");

            let quoted_indicator_name = format_ident!("{indicator_name}");
            let quoted_indicator_doc =
                format!("Indicator component for the [`{name}`] [`::re_types_core::Archetype`]");

            let (num_required, required) =
                compute_components(obj, ATTR_RERUN_COMPONENT_REQUIRED, []);
            let (num_recommended, recommended) =
                compute_components(obj, ATTR_RERUN_COMPONENT_RECOMMENDED, [indicator_fqname]);
            let (num_optional, optional) = compute_components(
                obj,
                ATTR_RERUN_COMPONENT_OPTIONAL,
                // NOTE: Our internal query systems always need to query for instance keys, and
                // they need to do so using a compile-time array, so make sure it's there at
                // compile-time even for archetypes that don't use it.
                ["rerun.components.InstanceKey".to_owned()],
            );

            let num_all = num_required + num_recommended + num_optional;

            let quoted_field_names = obj
                .fields
                .iter()
                .map(|field| format_ident!("{}", field.name))
                .collect::<Vec<_>>();

            let all_component_batches = {
                std::iter::once(quote!{
                    Some(Self::indicator())
                }).chain(obj.fields.iter().map(|obj_field| {
                    let field_name = format_ident!("{}", obj_field.name);
                    let is_plural = obj_field.typ.is_plural();
                    let is_nullable = obj_field.is_nullable;

                    // NOTE: Archetypes are AoS (arrays of structs), thus the nullability we're
                    // dealing with here is the nullability of an entire array of components, not
                    // the nullability of individual elements (i.e. instances)!
                    match (is_plural, is_nullable) {
                        (true, true) => quote! {
                            self.#field_name.as_ref().map(|comp_batch| (comp_batch as &dyn ComponentBatch).into())
                        },
                        (false, true) => quote! {
                            self.#field_name.as_ref().map(|comp| (comp as &dyn ComponentBatch).into())
                        },
                        (_, false) => quote! {
                            Some((&self.#field_name as &dyn ComponentBatch).into())
                        }
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
                    let quoted_deser = if is_nullable && !is_plural{
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
                                <#component>::from_arrow_opt(&**array)
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
                                    <#component>::from_arrow_opt(&**array)
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

                            <#component>::from_arrow_opt(&**array).with_context(#obj_field_fqname)? #quoted_collection
                        }}
                    };

                    quote!(let #field_name = #quoted_deser;)
                })
            };

            quote! {
                static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; #num_required]> =
                    once_cell::sync::Lazy::new(|| {[#required]});

                static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; #num_recommended]> =
                    once_cell::sync::Lazy::new(|| {[#recommended]});

                static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; #num_optional]> =
                    once_cell::sync::Lazy::new(|| {[#optional]});

                static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; #num_all]> =
                    once_cell::sync::Lazy::new(|| {[#required #recommended #optional]});

                impl #name {
                    pub const NUM_COMPONENTS: usize = #num_all;
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
                    fn indicator() -> MaybeOwnedComponentBatch<'static> {
                        static INDICATOR: #quoted_indicator_name = #quoted_indicator_name::DEFAULT;
                        MaybeOwnedComponentBatch::Ref(&INDICATOR)
                    }

                    #[inline]
                    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
                        REQUIRED_COMPONENTS.as_slice().into()
                    }

                    #[inline]
                    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentName]>  {
                        RECOMMENDED_COMPONENTS.as_slice().into()
                    }

                    #[inline]
                    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentName]>  {
                        OPTIONAL_COMPONENTS.as_slice().into()
                    }

                    // NOTE: Don't rely on default implementation so that we can keep everything static.
                    #[inline]
                    fn all_components() -> ::std::borrow::Cow<'static, [ComponentName]>  {
                        ALL_COMPONENTS.as_slice().into()
                    }

                    #[inline]
                    fn from_arrow_components(
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
                    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
                        re_tracing::profile_function!();

                        use ::re_types_core::Archetype as _;

                        [#(#all_component_batches,)*].into_iter().flatten().collect()
                    }

                    #[inline]
                    fn num_instances(&self) -> usize {
                        #num_instances
                    }
                }
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
            let quoted_type = quote_field_type_from_object_field(obj_field);

            let quoted_binding = if obj_is_tuple_struct {
                quote!(Self(v.into()))
            } else {
                quote!(Self { #quoted_obj_field_name: v.into() })
            };

            let quoted_borrow_deref_impl = if obj_is_tuple_struct {
                quote!(&self.0)
            } else {
                quote!( &self.#quoted_obj_field_name )
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
                        #quoted_borrow_deref_impl
                    }
                }

                impl std::ops::Deref for #quoted_obj_name {
                    type Target = #quoted_type;

                    #[inline]
                    fn deref(&self) -> &#quoted_type {
                        #quoted_borrow_deref_impl
                    }
                }
            }
        }
    } else {
        let quoted_type = quote_field_type_from_object_field(obj_field);
        let quoted_obj_field_name = format_ident!("{}", obj_field.name);

        let (quoted_binding, quoted_read) = if obj_is_tuple_struct {
            (quote!(Self(#quoted_obj_field_name)), quote!(value.0))
        } else {
            (
                quote!(Self { #quoted_obj_field_name }),
                quote!(value.#quoted_obj_field_name),
            )
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
        }
    }
}

/// Only makes sense for archetypes.
fn quote_builder_from_obj(obj: &Object) -> TokenStream {
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

    // --- impl new() ---

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
    let fn_new = quote! {
        #fn_new_pub fn new(#(#quoted_params,)*) -> Self {
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
        let (typ, unwrapped) = quote_field_type_from_typ(&field.typ, true);

        if unwrapped {
            // This was originally a vec/array!
            quote! {
                #[inline]
                pub fn #method_name(mut self, #field_name: impl IntoIterator<Item = impl Into<#typ>>) -> Self {
                    self.#field_name = Some(#field_name.into_iter().map(Into::into).collect());
                    self
                }
            }
        } else {
            quote! {
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
