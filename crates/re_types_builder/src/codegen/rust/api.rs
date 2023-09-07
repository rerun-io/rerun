use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rayon::prelude::*;

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
            util::{is_tuple_struct_from_obj, iter_archetype_components},
        },
        StringExt as _,
    },
    ArrowRegistry, CodeGenerator, Docs, ElementType, Object, ObjectField, ObjectKind, Objects,
    Type, ATTR_RERUN_COMPONENT_OPTIONAL, ATTR_RERUN_COMPONENT_RECOMMENDED,
    ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RERUN_LEGACY_FQNAME, ATTR_RUST_CUSTOM_CLAUSE,
    ATTR_RUST_DERIVE, ATTR_RUST_DERIVE_ONLY, ATTR_RUST_REPR,
};

use super::{arrow::quote_fqname_as_type_path, util::string_from_quoted};

// TODO(cmc): it'd be nice to be able to generate vanilla comments (as opposed to doc-comments)
// once again at some point (`TokenStream` strips them)… nothing too urgent though.

// ---

pub struct RustCodeGenerator {
    crate_path: Utf8PathBuf,
}

impl RustCodeGenerator {
    pub fn new(crate_path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            crate_path: crate_path.into(),
        }
    }
}

impl CodeGenerator for RustCodeGenerator {
    fn generate(
        &mut self,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
    ) -> BTreeSet<Utf8PathBuf> {
        let mut files_to_write: BTreeMap<Utf8PathBuf, String> = Default::default();

        let datatypes_path = self.crate_path.join("src/datatypes");
        let datatypes_testing_path = self.crate_path.join("src/testing/datatypes");
        files_to_write.extend(create_files(
            datatypes_path,
            datatypes_testing_path,
            arrow_registry,
            objects,
            &objects.ordered_objects(ObjectKind::Datatype.into()),
        ));

        let components_path = self.crate_path.join("src/components");
        let components_testing_path = self.crate_path.join("src/testing/components");
        files_to_write.extend(create_files(
            components_path,
            components_testing_path,
            arrow_registry,
            objects,
            &objects.ordered_objects(ObjectKind::Component.into()),
        ));

        let archetypes_path = self.crate_path.join("src/archetypes");
        let archetypes_testing_path = self.crate_path.join("src/testing/archetypes");
        files_to_write.extend(create_files(
            archetypes_path,
            archetypes_testing_path,
            arrow_registry,
            objects,
            &objects.ordered_objects(ObjectKind::Archetype.into()),
        ));

        write_files(&files_to_write);

        let filepaths = files_to_write.keys().cloned().collect();

        for kind in ObjectKind::ALL {
            let folder_path = self.crate_path.join("src").join(kind.plural_snake_case());
            crate::codegen::common::remove_old_files_from_folder(folder_path, &filepaths);
        }

        filepaths
    }
}

// --- File management ---

fn create_files(
    out_path: impl AsRef<Utf8Path>,
    out_testing_path: impl AsRef<Utf8Path>,
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    objs: &[&Object],
) -> BTreeMap<Utf8PathBuf, String> {
    let out_path = out_path.as_ref();
    let out_testing_path = out_testing_path.as_ref();

    let mut files_to_write = BTreeMap::new();

    let mut files = HashMap::<Utf8PathBuf, Vec<QuotedObject>>::new();
    for obj in objs {
        let quoted_obj = if obj.is_struct() {
            QuotedObject::from_struct(arrow_registry, objects, obj)
        } else {
            QuotedObject::from_union(arrow_registry, objects, obj)
        };

        let filepath = if quoted_obj.is_testing {
            out_testing_path.join(quoted_obj.filepath.file_name().unwrap())
        } else {
            out_path.join(quoted_obj.filepath.file_name().unwrap())
        };
        files.entry(filepath.clone()).or_default().push(quoted_obj);
    }

    // (module_name, [object_name])
    let mut mods = HashMap::<String, Vec<String>>::new();
    let mut mods_testing = HashMap::<String, Vec<String>>::new();

    // src/testing?/{datatypes|components|archetypes}/{xxx}.rs
    for (filepath, objs) in files {
        // NOTE: Isolating the file stem only works because we're handling datatypes, components
        // and archetypes separately (and even then it's a bit shady, eh).

        let names = objs
            .iter()
            .filter(|obj| !obj.is_testing)
            .map(|obj| obj.name.clone())
            .collect::<Vec<_>>();
        if !names.is_empty() {
            mods.entry(filepath.file_stem().unwrap().to_owned())
                .or_default()
                .extend(names);
        }

        let names_testing = objs
            .iter()
            .filter(|obj| obj.is_testing)
            .map(|obj| obj.name.clone())
            .collect::<Vec<_>>();
        if !names_testing.is_empty() {
            mods_testing
                .entry(filepath.file_stem().unwrap().to_owned())
                .or_default()
                .extend(names_testing);
        }

        let mut code = String::new();
        #[rustfmt::skip]
        {
            code.push_text(format!("// {}", autogen_warning!()), 2, 0);
            code.push_text("#![allow(trivial_numeric_casts)]", 2, 0);
            code.push_text("#![allow(unused_parens)]", 2, 0);
            code.push_text("#![allow(clippy::clone_on_copy)]", 2, 0);
            code.push_text("#![allow(clippy::iter_on_single_items)]", 2, 0);
            code.push_text("#![allow(clippy::map_flatten)]", 2, 0);
            code.push_text("#![allow(clippy::match_wildcard_for_single_variants)]", 2, 0);
            code.push_text("#![allow(clippy::needless_question_mark)]", 2, 0);
            code.push_text("#![allow(clippy::redundant_closure)]", 2, 0);
            code.push_text("#![allow(clippy::too_many_arguments)]", 2, 0);
            code.push_text("#![allow(clippy::too_many_lines)]", 2, 0);
            code.push_text("#![allow(clippy::unnecessary_cast)]", 2, 0);
        };

        for obj in objs {
            let mut acc = TokenStream::new();

            // NOTE: `TokenStream`s discard whitespacing information by definition, so we need to
            // inject some of our own when writing to file… while making sure that don't inject
            // random spacing into doc comments that look like code!

            let mut tokens = obj.tokens.into_iter();
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
        }

        code = replace_doc_attrb_with_doc_comment(&code);

        files_to_write.insert(filepath, code);
    }

    let mut generate_mod_files = |out_path: &Utf8Path, mods: &HashMap<String, Vec<String>>| {
        let path = out_path.join("mod.rs");

        let mut code = String::new();

        code.push_text(format!("// {}", autogen_warning!()), 2, 0);

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

        for (module, names) in mods {
            let names = names.join(", ");
            code.push_text(format!("pub use self::{module}::{{{names}}};"), 1, 0);
        }

        files_to_write.insert(path, code);
    };

    // src/{datatypes|components|archetypes}/mod.rs
    generate_mod_files(out_path, &mods);

    // src/testing/{datatypes|components|archetypes}/mod.rs
    generate_mod_files(out_testing_path, &mods_testing);

    files_to_write
}

fn write_files(files_to_write: &BTreeMap<Utf8PathBuf, String>) {
    re_tracing::profile_function!();
    // TODO(emilk): running `cargo fmt` once for each file is very slow.
    // It would probably be faster to write all filtes to a temporary folder, run carg-fmt on
    // that folder, and then copy the results to the final destination (if the files has changed).
    files_to_write.par_iter().for_each(|(path, source)| {
        write_file(path, source.clone());
    });
}

fn write_file(filepath: &Utf8PathBuf, mut code: String) {
    re_tracing::profile_function!();

    // Even though we already have used `prettyplease` we also
    // need to run `cargo fmt`, since it catches some things `prettyplease` missed.
    // We need to run `cago fmt` several times because it is not idempotent;
    // see https://github.com/rust-lang/rustfmt/issues/5824
    for _ in 0..2 {
        // NOTE: We're purposefully ignoring the error here.
        //
        // In the very unlikely chance that the user doesn't have the `fmt` component installed,
        // there's still no good reason to fail the build.
        //
        // The CI will catch the unformatted file at PR time and complain appropriately anyhow.

        re_tracing::profile_scope!("rust-fmt");
        use rust_format::Formatter as _;
        if let Ok(formatted) = rust_format::RustFmt::default().format_str(&code) {
            code = formatted;
        }
    }

    crate::codegen::common::write_file(filepath, code);
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
                new_code.push_str("/// ");
                unescape_string_into(&code[content_start..content_end], &mut new_code);
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

#[derive(Debug, Clone)]
struct QuotedObject {
    filepath: Utf8PathBuf,
    name: String,
    is_testing: bool,
    tokens: TokenStream,
}

impl QuotedObject {
    fn from_struct(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> Self {
        assert!(obj.is_struct());

        let Object {
            virtpath,
            filepath: _,
            fqname: _,
            pkg_name: _,
            name,
            docs,
            kind: _,
            attrs: _,
            order: _,
            fields,
            specifics: _,
            datatype: _,
        } = obj;

        let name = format_ident!("{name}");

        let quoted_doc = quote_doc_from_docs(docs);

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

        let quoted_fields = fields
            .iter()
            .map(|obj_field| ObjectFieldTokenizer(obj, obj_field));

        let is_tuple_struct = is_tuple_struct_from_obj(obj);
        let quoted_struct = if is_tuple_struct {
            quote! { pub struct #name(#(#quoted_fields,)*); }
        } else {
            quote! { pub struct #name { #(#quoted_fields,)* }}
        };

        let quoted_from_impl = quote_from_impl_from_obj(obj);

        let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, objects, obj);

        let quoted_builder = quote_builder_from_obj(obj);

        let tokens = quote! {
            #quoted_doc
            #quoted_derive_clone_debug
            #quoted_derive_clause
            #quoted_repr_clause
            #quoted_custom_clause
            #quoted_struct

            #quoted_from_impl

            #quoted_trait_impls

            #quoted_builder
        };

        Self {
            filepath: {
                let mut filepath = Utf8PathBuf::from(virtpath);
                filepath.set_extension("rs");
                filepath
            },
            name: obj.name.clone(),
            is_testing: obj.is_testing(),
            tokens,
        }
    }

    fn from_union(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> Self {
        assert!(!obj.is_struct());

        let Object {
            virtpath,
            filepath: _,
            fqname: _,
            pkg_name: _,
            name,
            docs,
            kind: _,
            attrs: _,
            order: _,
            fields,
            specifics: _,
            datatype: _,
        } = obj;

        let name = format_ident!("{name}");

        let quoted_doc = quote_doc_from_docs(docs);
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
            } = obj_field;

            let name = format_ident!("{}", crate::to_pascal_case(name));

            let quoted_doc = quote_doc_from_docs(docs);
            let (quoted_type, _) = quote_field_type_from_field(obj_field, false);
            let quoted_type = if *is_nullable {
                quote!(Option<#quoted_type>)
            } else {
                quoted_type
            };

            quote! {
                #quoted_doc
                #name(#quoted_type)
            }
        });

        let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, objects, obj);

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
        };

        Self {
            filepath: {
                let mut filepath = Utf8PathBuf::from(virtpath);
                filepath.set_extension("rs");
                filepath
            },
            name: obj.name.clone(),
            is_testing: obj.is_testing(),
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
            virtpath: _,
            filepath: _,
            pkg_name: _,
            fqname: _,
            name,
            docs,
            typ: _,
            attrs: _,
            order: _,
            is_nullable,
            // TODO(#2366): support for deprecation notices
            is_deprecated: _,
            datatype: _,
        } = obj_field;

        let quoted_docs = quote_doc_from_docs(docs);
        let name = format_ident!("{name}");

        let (quoted_type, _) = quote_field_type_from_field(obj_field, false);
        let quoted_type = if *is_nullable {
            quote!(Option<#quoted_type>)
        } else {
            quoted_type
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
            tokens.extend(self.0.iter().map(|line| quote!(# [doc = #line])));
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
    let obj_field_type = TypeTokenizer {
        typ: &obj_field.typ,
        unwrap,
    };
    let unwrapped = unwrap && matches!(obj_field.typ, Type::Array { .. } | Type::Vector { .. });
    (quote!(#obj_field_type), unwrapped)
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
            Type::Float16 => unimplemented!("{typ:#?}"),
            Type::Float32 => quote!(f32),
            Type::Float64 => quote!(f64),
            Type::String => quote!(crate::ArrowString),
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
                    quote!(crate::ArrowBuffer<#elem_type>)
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
            ElementType::Float16 => unimplemented!("{self:#?}"),
            ElementType::Float32 => quote!(f32),
            ElementType::Float64 => quote!(f64),
            ElementType::String => quote!(crate::ArrowString),
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
        virtpath: _,
        filepath: _,
        fqname,
        pkg_name: _,
        name,
        docs: _,
        kind,
        attrs: _,
        order: _,
        fields: _,
        specifics: _,
        datatype: _,
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

            let legacy_fqname = obj
                .try_get_attr::<String>(ATTR_RERUN_LEGACY_FQNAME)
                .unwrap_or_else(|| fqname.clone());

            let quoted_serializer =
                quote_arrow_serializer(arrow_registry, objects, obj, &format_ident!("data"));
            let quoted_deserializer = quote_arrow_deserializer(arrow_registry, objects, obj);

            let into_cow = quote! {
                // NOTE: We need these so end-user code can effortlessly serialize both iterators
                // of owned data and iterators of referenced data without ever having to stop and
                // think about it.

                impl<'a> From<#name> for ::std::borrow::Cow<'a, #name> {
                    #[inline]
                    fn from(value: #name) -> Self {
                        std::borrow::Cow::Owned(value)
                    }
                }

                impl<'a> From<&'a #name> for ::std::borrow::Cow<'a, #name> {
                    #[inline]
                    fn from(value: &'a #name) -> Self {
                        std::borrow::Cow::Borrowed(value)
                    }
                }
            };

            let quoted_try_from_arrow = if optimize_for_buffer_slice {
                let quoted_deserializer =
                    quote_arrow_deserializer_buffer_slice(arrow_registry, objects, obj);
                quote! {
                    #[allow(unused_imports, clippy::wildcard_imports)]
                    #[inline]
                    fn try_from_arrow(arrow_data: &dyn ::arrow2::array::Array) -> crate::DeserializationResult<Vec<Self>>
                    where
                        Self: Sized {
                        use ::arrow2::{datatypes::*, array::*, buffer::*};
                        use crate::{Loggable as _, ResultExt as _};

                        // This code-path cannot have null fields. If it does have a validity mask
                        // all bits must indicate valid data.
                        if let Some(validity) = arrow_data.validity() {
                            if validity.unset_bits() != 0 {
                                return Err(crate::DeserializationError::missing_data());
                            }
                        }

                        Ok(#quoted_deserializer)
                    }
                }
            } else {
                quote!()
            };

            quote! {
                #into_cow

                impl crate::Loggable for #name {
                    type Name = crate::#kind_name;

                    #[inline]
                    fn name() -> Self::Name {
                        #legacy_fqname.into()
                    }

                    #[allow(unused_imports, clippy::wildcard_imports)]
                    #[inline]
                    fn arrow_datatype() -> arrow2::datatypes::DataType {
                        use ::arrow2::datatypes::*;
                        #datatype
                    }

                    // NOTE: Don't inline this, this gets _huge_.
                    #[allow(unused_imports, clippy::wildcard_imports)]
                    fn try_to_arrow_opt<'a>(
                        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
                    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
                    where
                        Self: Clone + 'a
                    {
                        use ::arrow2::{datatypes::*, array::*};
                        use crate::{Loggable as _, ResultExt as _};
                        Ok(#quoted_serializer)
                    }

                    // NOTE: Don't inline this, this gets _huge_.
                    #[allow(unused_imports, clippy::wildcard_imports)]
                    fn try_from_arrow_opt(arrow_data: &dyn ::arrow2::array::Array) -> crate::DeserializationResult<Vec<Option<Self>>>
                    where
                        Self: Sized
                    {
                        use ::arrow2::{datatypes::*, array::*, buffer::*};
                        use crate::{Loggable as _, ResultExt as _};
                        Ok(#quoted_deserializer)
                    }

                    #quoted_try_from_arrow
                }
            }
        }

        ObjectKind::Archetype => {
            fn compute_components(
                obj: &Object,
                attr: &'static str,
                objects: &Objects,
            ) -> (usize, TokenStream) {
                let components = iter_archetype_components(obj, attr)
                    .map(|fqname| {
                        objects[fqname.as_str()]
                            .try_get_attr::<String>(crate::ATTR_RERUN_LEGACY_FQNAME)
                            .unwrap_or(fqname)
                    })
                    .collect::<Vec<_>>();
                let num_components = components.len();
                let quoted_components = quote!(#(#components.into(),)*);
                (num_components, quoted_components)
            }

            let first_required_comp = obj
                .fields
                .iter()
                .find(|field| {
                    field
                        .try_get_attr::<String>(ATTR_RERUN_COMPONENT_REQUIRED)
                        .is_some()
                })
                // NOTE: must have at least one required component.
                .unwrap();

            let num_instances = if first_required_comp.typ.is_plural() {
                let name = format_ident!("{}", first_required_comp.name);
                quote!(self.#name.len())
            } else {
                quote!(1)
            };

            let (num_required, required) =
                compute_components(obj, ATTR_RERUN_COMPONENT_REQUIRED, objects);
            let (num_recommended, recommended) =
                compute_components(obj, ATTR_RERUN_COMPONENT_RECOMMENDED, objects);
            let (num_optional, optional) =
                compute_components(obj, ATTR_RERUN_COMPONENT_OPTIONAL, objects);

            let num_all = num_required + num_recommended + num_optional;

            let quoted_field_names = obj
                .fields
                .iter()
                .map(|field| format_ident!("{}", field.name))
                .collect::<Vec<_>>();

            let all_component_lists = {
                obj.fields.iter().map(|obj_field| {
                    let field_name = format_ident!("{}", obj_field.name);
                    let is_plural = obj_field.typ.is_plural();
                    let is_nullable = obj_field.is_nullable;

                    // NOTE: Archetypes are AoS (arrays of structs), thus the nullability we're
                    // dealing with here is the nullability of an entire array of components, not
                    // the nullability of individual elements (i.e. instances)!
                    match (is_plural, is_nullable) {
                        (true, true) => {
                            quote! { self.#field_name.as_ref().map(|comp_list| comp_list as &dyn crate::ComponentList) }
                        }
                        (false, true) => {
                            quote! { self.#field_name.as_ref().map(|comp| comp as &dyn crate::ComponentList) }
                        }
                        (_, false) => {
                            quote! { Some(&self.#field_name as &dyn crate::ComponentList) }
                        }
                    }
                })
            };

            let all_serializers = {
                obj.fields.iter().map(|obj_field| {
                    let obj_field_fqname = obj_field.fqname.as_str();
                    let field_name_str = &obj_field.name;
                    let field_name = format_ident!("{}", obj_field.name);

                    let is_plural = obj_field.typ.is_plural();
                    let is_nullable = obj_field.is_nullable;

                    // NOTE: unwrapping is safe since the field must point to a component.
                    let component = quote_fqname_as_type_path(obj_field.typ.fqname().unwrap());

                    let fqname = obj_field.typ.fqname().unwrap();
                    let legacy_fqname = objects[fqname]
                        .try_get_attr::<String>(crate::ATTR_RERUN_LEGACY_FQNAME)
                        .unwrap_or_else(|| fqname.to_owned());

                    let extract_datatype_and_return = quote! {
                        array.map(|array| {
                            // NOTE: Temporarily injecting the extension metadata as well as the
                            // legacy fully-qualified name into the `Field` object so we can work
                            // around `arrow2-convert` limitations and map to old names while we're
                            // migrating.
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                #fqname.into(),
                                Box::new(array.data_type().clone()),
                                Some(#legacy_fqname.into()),
                            );
                            (::arrow2::datatypes::Field::new(#field_name_str, datatype, false), array)
                        })
                    };

                    // NOTE: Archetypes are AoS (arrays of structs), thus the nullability we're
                    // dealing with here is the nullability of an entire array of components, not
                    // the nullability of individual elements (i.e. instances)!
                    match (is_plural, is_nullable) {
                        (true, true) => quote! {
                             self.#field_name.as_ref().map(|many| {
                                let array = <#component>::try_to_arrow(many.iter());
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .with_context(#obj_field_fqname)?
                        },
                        (true, false) => quote! {
                            Some({
                                let array = <#component>::try_to_arrow(self.#field_name.iter());
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .with_context(#obj_field_fqname)?
                        },
                        (false, true) => quote! {
                             self.#field_name.as_ref().map(|single| {
                                let array = <#component>::try_to_arrow([single]);
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .with_context(#obj_field_fqname)?
                        },
                        (false, false) => quote! {
                            Some({
                                let array = <#component>::try_to_arrow([&self.#field_name]);
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .with_context(#obj_field_fqname)?
                        },
                    }
                })
            };

            let all_deserializers = {
                obj.fields.iter().map(|obj_field| {
                    let obj_field_fqname = obj_field.fqname.as_str();
                    let field_name_str = &obj_field.name;
                    let field_name = format_ident!("{}", obj_field.name);

                    let is_plural = obj_field.typ.is_plural();
                    let is_nullable = obj_field.is_nullable;

                    // NOTE: unwrapping is safe since the field must point to a component.
                    let component = quote_fqname_as_type_path(obj_field.typ.fqname().unwrap());

                    let quoted_collection = if is_plural {
                        quote! {
                            .into_iter()
                            .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
                            .collect::<crate::DeserializationResult<Vec<_>>>()
                            .with_context(#obj_field_fqname)?
                        }
                    } else {
                        quote! {
                            .into_iter()
                            .next()
                            .flatten()
                            .ok_or_else(crate::DeserializationError::missing_data)
                            .with_context(#obj_field_fqname)?
                        }
                    };

                    let quoted_deser = if is_nullable {
                        quote! {
                            if let Some(array) = arrays_by_name.get(#field_name_str) {
                                Some({
                                    <#component>::try_from_arrow_opt(&**array)
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
                                .get(#field_name_str)
                                .ok_or_else(crate::DeserializationError::missing_data)
                                .with_context(#obj_field_fqname)?;

                            <#component>::try_from_arrow_opt(&**array).with_context(#obj_field_fqname)? #quoted_collection
                        }}
                    };

                    quote!(let #field_name = #quoted_deser;)
                })
            };

            let indicator_fqname =
                format!("{}Indicator", obj.fqname).replace("rerun.archetypes", "rerun.components");

            quote! {
                static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_required]> =
                    once_cell::sync::Lazy::new(|| {[#required]});

                static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_recommended]> =
                    once_cell::sync::Lazy::new(|| {[#recommended]});

                static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_optional]> =
                    once_cell::sync::Lazy::new(|| {[#optional]});

                static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_all]> =
                    once_cell::sync::Lazy::new(|| {[#required #recommended #optional]});

                impl #name {
                    pub const NUM_COMPONENTS: usize = #num_all;
                }

                impl crate::Archetype for #name {
                    #[inline]
                    fn name() -> crate::ArchetypeName {
                        #fqname.into()
                    }

                    #[inline]
                    fn required_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
                        REQUIRED_COMPONENTS.as_slice().into()
                    }

                    #[inline]
                    fn recommended_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]>  {
                        RECOMMENDED_COMPONENTS.as_slice().into()
                    }

                    #[inline]
                    fn optional_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]>  {
                        OPTIONAL_COMPONENTS.as_slice().into()
                    }

                    // NOTE: Don't rely on default implementation so that we can keep everything static.
                    #[inline]
                    fn all_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]>  {
                        ALL_COMPONENTS.as_slice().into()
                    }

                    // NOTE: Don't rely on default implementation so that we can avoid runtime formatting.
                    #[inline]
                    fn indicator_component() -> crate::ComponentName  {
                        #indicator_fqname.into()
                    }

                    #[inline]
                    fn num_instances(&self) -> usize {
                        #num_instances
                    }

                    fn as_component_lists(&self) -> Vec<&dyn crate::ComponentList> {
                        [#(#all_component_lists,)*].into_iter().flatten().collect()
                    }

                    // TODO(#3159): Make indicator components first class and return them through `as_component_lists`,
                    // at which point we can rely on the default implementation and remove this altogether.
                    #[inline]
                    fn try_to_arrow(
                        &self,
                    ) -> crate::SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>> {
                        use crate::{Loggable as _, ResultExt as _};
                        Ok([
                            #({ #all_serializers }),*,
                            // Inject the indicator component.
                            {
                                let datatype = ::arrow2::datatypes::DataType::Extension(
                                    #indicator_fqname.to_owned(),
                                    Box::new(::arrow2::datatypes::DataType::Null),
                                    // NOTE: Mandatory during migration to codegen.
                                    Some(#indicator_fqname.to_owned()),
                                );
                                let array = ::arrow2::array::NullArray::new(
                                    datatype.to_logical_type().clone(), self.num_instances(),
                                ).boxed();
                                Some((
                                    ::arrow2::datatypes::Field::new(#indicator_fqname, datatype, false),
                                    array,
                                ))
                            },
                        ].into_iter().flatten().collect())
                    }

                    #[inline]
                    fn try_from_arrow(
                        arrow_data: impl IntoIterator<Item = (::arrow2::datatypes::Field, Box<dyn::arrow2::array::Array>)>,
                    ) -> crate::DeserializationResult<Self> {
                        use crate::{Loggable as _, ResultExt as _};

                        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
                            .into_iter()
                            .map(|(field, array)| (field.name, array))
                            .collect();

                        #(#all_deserializers;)*

                        Ok(Self {
                            #(#quoted_field_names,)*
                        })
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

    let obj_field = &obj.fields[0];
    if obj_field.typ.fqname().is_some() {
        let quoted_obj_name = format_ident!("{}", obj.name);
        let (quoted_type, _) = quote_field_type_from_field(&obj.fields[0], false);

        if let Some(inner) = obj_field.typ.vector_inner() {
            if obj_field.is_nullable {
                quote! {
                    impl<I: Into<#inner>, T: IntoIterator<Item = I>> From<Option<T>> for #quoted_obj_name {
                        fn from(v: Option<T>) -> Self {
                            Self(v.map(|v| v.into_iter().map(|v| v.into()).collect()))
                        }
                    }
                }
            } else {
                quote! {
                    impl<I: Into<#inner>, T: IntoIterator<Item = I>> From<T> for #quoted_obj_name {
                        fn from(v: T) -> Self {
                            Self(v.into_iter().map(|v| v.into()).collect())
                        }
                    }
                }
            }
        } else {
            let quoted_type = if obj_field.is_nullable {
                quote!(Option<#quoted_type>)
            } else {
                quote!(#quoted_type)
            };

            let obj_is_tuple_struct = is_tuple_struct_from_obj(obj);
            let quoted_binding = if obj_is_tuple_struct {
                quote!(Self(v.into()))
            } else {
                let quoted_obj_field_name = format_ident!("{}", obj_field.name);
                quote!(Self { #quoted_obj_field_name: v.into() })
            };

            quote! {
                impl<T: Into<#quoted_type>> From<T> for #quoted_obj_name {
                    fn from(v: T) -> Self {
                        #quoted_binding
                    }
                }
            }
        }
    } else {
        quote!()
    }
}

/// Only makes sense for archetypes.
fn quote_builder_from_obj(obj: &Object) -> TokenStream {
    if obj.kind != ObjectKind::Archetype {
        return TokenStream::new();
    }

    let Object {
        virtpath: _,
        filepath: _,
        fqname: _,
        pkg_name: _,
        name,
        docs: _,
        kind: _,
        attrs: _,
        order: _,
        fields,
        specifics: _,
        datatype: _,
    } = obj;

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
