//! Implements the Rust codegen pass.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Context as _;
use arrow2::datatypes::DataType;
use camino::{Utf8Path, Utf8PathBuf};
use convert_case::{Case, Casing as _};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rayon::prelude::*;

use crate::{
    codegen::{StringExt as _, AUTOGEN_WARNING},
    ArrowRegistry, CodeGenerator, Docs, ElementType, Object, ObjectField, ObjectKind, Objects,
    Type, ATTR_RERUN_COMPONENT_OPTIONAL, ATTR_RERUN_COMPONENT_RECOMMENDED,
    ATTR_RERUN_COMPONENT_REQUIRED, ATTR_RERUN_LEGACY_FQNAME, ATTR_RUST_CUSTOM_CLAUSE,
    ATTR_RUST_DERIVE, ATTR_RUST_REPR, ATTR_RUST_TUPLE_STRUCT,
};

// TODO(cmc): it'd be nice to be able to generate vanilla comments (as opposed to doc-comments)
// once again at some point (`TokenStream` strips them)... nothing too urgent though.

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
            super::common::remove_old_files_from_folder(folder_path, &filepaths);
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
            code.push_text(format!("// {AUTOGEN_WARNING}"), 2, 0);
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
            // inject some of our own when writing to file... while making sure that don't inject
            // random spacing into doc comments that look like code!

            let mut tokens = obj.tokens.into_iter();
            while let Some(token) = tokens.next() {
                match &token {
                    // If this is a doc-comment block, be smart about it.
                    proc_macro2::TokenTree::Punct(punct) if punct.as_char() == '#' => {
                        let tokens_str = acc
                            .to_string()
                            .replace('}', "}\n\n")
                            .replace("] ;", "];\n\n")
                            .replace("# [doc", "\n\n# [doc")
                            .replace("impl ", "\n\nimpl ");
                        code.push_text(tokens_str, 1, 0);
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

            let tokens_str = acc
                .to_string()
                .replace('}', "}\n\n")
                .replace("] ;", "];\n\n")
                .replace("# [doc", "\n\n# [doc")
                .replace("impl ", "\n\nimpl ");

            code.push_text(tokens_str, 1, 0);
        }

        code = replace_doc_attrb_with_doc_comment(&code);

        files_to_write.insert(filepath, code);
    }

    let mut generate_mod_files = |out_path: &Utf8Path, mods: &HashMap<String, Vec<String>>| {
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

    // We need to run `cago fmt` several times because it is not idempotent!
    // See https://github.com/rust-lang/rustfmt/issues/5824
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

    super::common::write_file(filepath, code);
}

/// Replace `#[doc = "…"]` attributes with `/// …` doc comments,
/// while also removing trailing whitespace.
fn replace_doc_attrb_with_doc_comment(code: &String) -> String {
    // This is difficult to do with regex, because the patterns with newlines overlap.

    let start_pattern = "# [doc = \"";
    let end_pattern = "\"]"; // assues there is no escaped quote followed by a bracket

    let problematic = r#"\"]"#;
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
        let quoted_derive_clone_debug = quote_derive_clone_debug();
        let quoted_derive_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE, "derive");
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
            is_testing: obj.fqname.contains("rerun.testing"),
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
        let quoted_derive_clone_debug = quote_derive_clone_debug();
        let quoted_derive_clause = quote_meta_clause_from_obj(obj, ATTR_RUST_DERIVE, "derive");
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

            let name = format_ident!("{}", name.to_case(Case::UpperCamel));

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
            is_testing: obj.fqname.contains("rerun.testing"),
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
            ElementType::Float16 => unimplemented!("{self:#?}"),
            ElementType::Float32 => quote!(f32),
            ElementType::Float64 => quote!(f64),
            ElementType::String => quote!(String),
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
            let kind = if *kind == ObjectKind::Datatype {
                quote!(Datatype)
            } else {
                quote!(Component)
            };
            let kind_name = format_ident!("{kind}Name");

            let datatype = arrow_registry.get(fqname);
            let datatype = ArrowDataTypeTokenizer(&datatype, false);

            let legacy_fqname = obj
                .try_get_attr::<String>(ATTR_RERUN_LEGACY_FQNAME)
                .unwrap_or_else(|| fqname.clone());

            let quoted_serializer =
                quote_arrow_serializer(arrow_registry, objects, obj, &format_ident!("data"));
            let quoted_deserializer =
                quote_arrow_deserializer(arrow_registry, objects, obj, &format_ident!("data"));

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

            quote! {
                #into_cow

                impl crate::Loggable for #name {
                    type Name = crate::#kind_name;
                    type Item<'a> = Option<Self>;
                    type Iter<'a> = Box<dyn Iterator<Item = Self::Item<'a>> + 'a>;

                    #[inline]
                    fn name() -> Self::Name {
                        #legacy_fqname.into()
                    }

                    #[allow(unused_imports, clippy::wildcard_imports)]
                    #[inline]
                    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
                        use ::arrow2::datatypes::*;
                        #datatype
                    }

                    // NOTE: Don't inline this, this gets _huge_.
                    #[allow(unused_imports, clippy::wildcard_imports)]
                    fn try_to_arrow_opt<'a>(
                        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
                        extension_wrapper: Option<&str>,
                    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
                    where
                        Self: Clone + 'a
                    {
                        use ::arrow2::{datatypes::*, array::*};
                        use crate::Loggable as _;
                        Ok(#quoted_serializer)
                    }

                    // NOTE: Don't inline this, this gets _huge_.
                    #[allow(unused_imports, clippy::wildcard_imports)]
                    fn try_from_arrow_opt(data: &dyn ::arrow2::array::Array) -> crate::DeserializationResult<Vec<Option<Self>>>
                    where
                        Self: Sized {
                        use ::arrow2::{datatypes::*, array::*};
                        use crate::Loggable as _;
                        Ok(#quoted_deserializer)
                    }


                    #[inline]
                    fn try_iter_from_arrow(
                        data: &dyn ::arrow2::array::Array,
                    ) -> crate::DeserializationResult<Self::Iter<'_>>
                    where
                        Self: Sized,
                    {
                        Ok(Box::new(Self::try_from_arrow_opt(data)?.into_iter()))
                    }

                    #[inline]
                    fn convert_item_to_self(item: Self::Item<'_>) -> Option<Self> {
                        item
                    }
                }

                impl crate::#kind for #name {}
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
                                let array = <#component>::try_to_arrow(many.iter(), None);
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .map_err(|err| crate::SerializationError::Context {
                                location: #obj_field_fqname.into(),
                                source: Box::new(err),
                            })?
                        },
                        (true, false) => quote! {
                            Some({
                                let array = <#component>::try_to_arrow(self.#field_name.iter(), None);
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .map_err(|err| crate::SerializationError::Context {
                                location: #obj_field_fqname.into(),
                                source: Box::new(err),
                            })?
                        },
                        (false, true) => quote! {
                             self.#field_name.as_ref().map(|single| {
                                let array = <#component>::try_to_arrow([single], None);
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .map_err(|err| crate::SerializationError::Context {
                                location: #obj_field_fqname.into(),
                                source: Box::new(err),
                            })?
                        },
                        (false, false) => quote! {
                            Some({
                                let array = <#component>::try_to_arrow([&self.#field_name], None);
                                #extract_datatype_and_return
                            })
                            .transpose()
                            .map_err(|err| crate::SerializationError::Context {
                                location: #obj_field_fqname.into(),
                                source: Box::new(err),
                            })?
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
                            .map(|v| v .ok_or_else(|| crate::DeserializationError::MissingData {
                                backtrace: ::backtrace::Backtrace::new_unresolved(),
                            }))
                            .collect::<crate::DeserializationResult<Vec<_>>>()
                            .map_err(|err| crate::DeserializationError::Context {
                                location: #obj_field_fqname.into(),
                                source: Box::new(err),
                            })?
                        }
                    } else {
                        quote! {
                            .into_iter()
                            .next()
                            .flatten()
                            .ok_or_else(|| crate::DeserializationError::MissingData {
                                backtrace: ::backtrace::Backtrace::new_unresolved(),
                            })
                            .map_err(|err| crate::DeserializationError::Context {
                                location: #obj_field_fqname.into(),
                                source: Box::new(err),
                            })?
                        }
                    };

                    let quoted_deser = if is_nullable {
                        quote! {
                            if let Some(array) = arrays_by_name.get(#field_name_str) {
                                Some(
                                    <#component>::try_from_arrow_opt(&**array)
                                        .map_err(|err| crate::DeserializationError::Context {
                                            location: #obj_field_fqname.into(),
                                            source: Box::new(err),
                                        })?
                                        #quoted_collection
                                )
                            } else {
                                None
                            }
                        }
                    } else {
                        quote! {{
                            let array = arrays_by_name
                                .get(#field_name_str)
                                .ok_or_else(|| crate::DeserializationError::MissingData {
                                    backtrace: ::backtrace::Backtrace::new_unresolved(),
                                })
                                .map_err(|err| crate::DeserializationError::Context {
                                    location: #obj_field_fqname.into(),
                                    source: Box::new(err),
                                })?;
                            <#component>::try_from_arrow_opt(&**array)
                                .map_err(|err| crate::DeserializationError::Context {
                                    location: #obj_field_fqname.into(),
                                    source: Box::new(err),
                                })?
                                #quoted_collection
                        }}
                    };

                    quote!(let #field_name = #quoted_deser;)
                })
            };

            quote! {
                static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_required]> = once_cell::sync::Lazy::new(|| {[#required]});

                static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_recommended]> = once_cell::sync::Lazy::new(|| {[#recommended]});

                static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_optional]> = once_cell::sync::Lazy::new(|| {[#optional]});

                static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; #num_all]> = once_cell::sync::Lazy::new(|| {[#required #recommended #optional]});

                impl #name {
                    pub const NUM_COMPONENTS: usize = #num_all;
                }

                impl crate::Archetype for #name {
                    #[inline]
                    fn name() -> crate::ArchetypeName {
                        crate::ArchetypeName::Borrowed(#fqname)
                    }

                    #[inline]
                    fn required_components() -> &'static [crate::ComponentName] {
                        REQUIRED_COMPONENTS.as_slice()
                    }

                    #[inline]
                    fn recommended_components() -> &'static [crate::ComponentName]  {
                        RECOMMENDED_COMPONENTS.as_slice()
                    }

                    #[inline]
                    fn optional_components() -> &'static [crate::ComponentName]  {
                        OPTIONAL_COMPONENTS.as_slice()
                    }

                    #[inline]
                    fn all_components() -> &'static [crate::ComponentName]  {
                        ALL_COMPONENTS.as_slice()
                    }

                    #[inline]
                    fn try_to_arrow(
                        &self,
                    ) -> crate::SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>> {
                        use crate::Loggable as _;
                        Ok([ #({ #all_serializers },)* ].into_iter().flatten().collect())
                    }

                    #[inline]
                    fn try_from_arrow(
                        data: impl IntoIterator<Item = (::arrow2::datatypes::Field, Box<dyn::arrow2::array::Array>)>,
                    ) -> crate::DeserializationResult<Self> {
                        use crate::Loggable as _;

                        let arrays_by_name: ::std::collections::HashMap<_, _> = data
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

// --- Arrow registry code generators ---

/// `(Datatype, is_recursive)`
struct ArrowDataTypeTokenizer<'a>(&'a ::arrow2::datatypes::DataType, bool);

impl quote::ToTokens for ArrowDataTypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use arrow2::datatypes::UnionMode;
        let Self(datatype, recursive) = self;
        match datatype {
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

            DataType::List(field) => {
                let field = ArrowFieldTokenizer(field);
                quote!(DataType::List(Box::new(#field)))
            }

            DataType::FixedSizeList(field, length) => {
                let field = ArrowFieldTokenizer(field);
                quote!(DataType::FixedSizeList(Box::new(#field), #length))
            }

            DataType::Union(fields, types, mode) => {
                let fields = fields.iter().map(ArrowFieldTokenizer);
                let mode = match mode {
                    UnionMode::Dense => quote!(UnionMode::Dense),
                    UnionMode::Sparse => quote!(UnionMode::Sparse),
                };
                if let Some(types) = types {
                    quote!(DataType::Union(vec![ #(#fields,)* ], Some(vec![ #(#types,)* ]), #mode))
                } else {
                    quote!(DataType::Union(vec![ #(#fields,)* ], None, #mode))
                }
            }

            DataType::Struct(fields) => {
                let fields = fields.iter().map(ArrowFieldTokenizer);
                quote!(DataType::Struct(vec![ #(#fields,)* ]))
            }

            DataType::Extension(fqname, datatype, _metadata) => {
                if *recursive {
                    let fqname_use = quote_fqname_as_type_path(fqname);
                    quote!(<#fqname_use>::to_arrow_datatype())
                } else {
                    let datatype = ArrowDataTypeTokenizer(datatype.to_logical_type(), false);
                    quote!(#datatype)
                    // TODO(cmc): Bring back extensions once we've fully replaced `arrow2-convert`!
                    // let datatype = ArrowDataTypeTokenizer(datatype, false);
                    // let metadata = OptionTokenizer(metadata.as_ref());
                    // quote!(DataType::Extension(#fqname.to_owned(), Box::new(#datatype), #metadata))
                }
            }

            _ => unimplemented!("{:#?}", self.0),
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

        let datatype = ArrowDataTypeTokenizer(data_type, true);
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

// --- Serialization ---

fn quote_arrow_serializer(
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    let datatype = &arrow_registry.get(&obj.fqname);

    let DataType::Extension(fqname, _, _) = datatype else { unreachable!() };
    let fqname_use = quote_fqname_as_type_path(fqname);
    let quoted_datatype = quote! {
        (if let Some(ext) = extension_wrapper {
            DataType::Extension(ext.to_owned(), Box::new(<#fqname_use>::to_arrow_datatype()), None)
        } else {
            <#fqname_use>::to_arrow_datatype()
        })
        // TODO(cmc): Bring back extensions once we've fully replaced `arrow2-convert`!
        .to_logical_type().clone()
    };

    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    let quoted_flatten = |obj_field_is_nullable| {
        // NOTE: If the field itself is marked nullable, then we'll end up with two layers of
        // nullability in the output. Get rid of the superfluous one.
        if obj_field_is_nullable {
            quote!(.flatten())
        } else {
            quote!()
        }
    };

    let quoted_bitmap = |var| {
        quote! {
            let #var: Option<::arrow2::bitmap::Bitmap> = {
                // NOTE: Don't compute a bitmap if there isn't at least one null element.
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            }
        }
    };

    if is_arrow_transparent {
        // NOTE: Arrow transparent objects must have a single field, no more no less.
        // The semantic pass would have failed already if this wasn't the case.
        let obj_field = &obj.fields[0];

        let quoted_data_src = data_src.clone();
        let quoted_data_dst = format_ident!(
            "{}",
            if is_tuple_struct {
                "data0"
            } else {
                obj_field.name.as_str()
            }
        );
        let bitmap_dst = format_ident!("{quoted_data_dst}_bitmap");

        let quoted_binding = if is_tuple_struct {
            quote!(Self(#quoted_data_dst))
        } else {
            quote!(Self { #quoted_data_dst })
        };

        let quoted_serializer = quote_arrow_field_serializer(
            objects,
            Some(obj.fqname.as_str()),
            &arrow_registry.get(&obj_field.fqname),
            obj_field.is_nullable,
            &bitmap_dst,
            &quoted_data_dst,
        );

        let quoted_bitmap = quoted_bitmap(bitmap_dst);

        let quoted_flatten = quoted_flatten(obj_field.is_nullable);

        quote! {{
            let (somes, #quoted_data_dst): (Vec<_>, Vec<_>) = #quoted_data_src
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);

                    let datum = datum
                        .map(|datum| {
                            let #quoted_binding = datum.into_owned();
                            #quoted_data_dst
                        })
                        #quoted_flatten;

                    (datum.is_some(), datum)
                })
                .unzip();


            #quoted_bitmap;

            #quoted_serializer
        }}
    } else {
        let data_src = data_src.clone();

        // NOTE: This can only be struct or union/enum at this point.
        match datatype.to_logical_type() {
            DataType::Struct(_) => {
                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let data_dst = format_ident!("{}", obj_field.name);
                    let bitmap_dst = format_ident!("{data_dst}_bitmap");

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        None,
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
                        &bitmap_dst,
                        &data_dst,
                    );

                    let quoted_flatten = quoted_flatten(obj_field.is_nullable);

                    let quoted_bitmap = quoted_bitmap(bitmap_dst);

                    quote! {{
                        let (somes, #data_dst): (Vec<_>, Vec<_>) = #data_src
                            .iter()
                            .map(|datum| {
                                let datum = datum
                                    .as_ref()
                                    .map(|datum| {
                                        let Self { #data_dst, .. } = &**datum;
                                        #data_dst.clone()
                                    })
                                    #quoted_flatten;

                                (datum.is_some(), datum)
                            })
                            .unzip();


                        #quoted_bitmap;

                        #quoted_serializer
                    }}
                });

                let quoted_bitmap = quoted_bitmap(format_ident!("bitmap"));

                quote! {{
                    let (somes, #data_src): (Vec<_>, Vec<_>) = #data_src
                        .into_iter()
                        .map(|datum| {
                            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                            (datum.is_some(), datum)
                        })
                        .unzip();

                    #quoted_bitmap;

                    StructArray::new(
                        #quoted_datatype,
                        vec![#(#quoted_field_serializers,)*],
                        bitmap,
                    ).boxed()
                }}
            }

            DataType::Union(_, _, arrow2::datatypes::UnionMode::Dense) => {
                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let data_dst = format_ident!("{}", obj_field.name.to_case(Case::Snake));
                    let bitmap_dst = format_ident!("{data_dst}_bitmap");

                    let quoted_serializer = quote_arrow_field_serializer(
                        objects,
                        None,
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
                        &bitmap_dst,
                        &data_dst,
                    );

                    let quoted_flatten = quoted_flatten(obj_field.is_nullable);
                    let quoted_bitmap = quoted_bitmap(bitmap_dst);

                    let quoted_obj_name = format_ident!("{}", obj.name);
                    let quoted_obj_field_name = format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));

                    quote! {{
                        let (somes, #data_dst): (Vec<_>, Vec<_>) = #data_src
                            .iter()
                            .filter(|datum| matches!(datum.as_deref(), Some(#quoted_obj_name::#quoted_obj_field_name(_))))
                            .map(|datum| {
                                let datum = match datum.as_deref() {
                                    Some(#quoted_obj_name::#quoted_obj_field_name(v)) => Some(v.clone()),
                                    _ => None,
                                } #quoted_flatten ;

                                (datum.is_some(), datum)
                            })
                            .unzip();


                        #quoted_bitmap;

                        #quoted_serializer
                    }}
                });

                let quoted_types = {
                    let quoted_obj_name = format_ident!("{}", obj.name);
                    let quoted_branches = obj.fields.iter().enumerate().map(|(i, obj_field)| {
                        let i = i as i8 + 1; // NOTE: +1 to account for `nulls` virtual arm
                        let quoted_obj_field_name =
                            format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));

                        quote!(Some(#quoted_obj_name::#quoted_obj_field_name(_)) => #i)
                    });

                    quote! {
                        #data_src
                            .iter()
                            .map(|a| match a.as_deref() {
                                None => 0,
                                #(#quoted_branches,)*
                            })
                            .collect()
                    }
                };

                let quoted_offsets = {
                    let quoted_obj_name = format_ident!("{}", obj.name);

                    let quoted_counters = obj.fields.iter().map(|obj_field| {
                        let quoted_obj_field_name =
                            format_ident!("{}_offset", obj_field.name.to_case(Case::Snake));
                        quote!(let mut #quoted_obj_field_name = 0)
                    });

                    let quoted_branches = obj.fields.iter().map(|obj_field| {
                        let quoted_counter_name =
                            format_ident!("{}_offset", obj_field.name.to_case(Case::Snake));
                        let quoted_obj_field_name =
                            format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));
                        quote! {
                            Some(#quoted_obj_name::#quoted_obj_field_name(_)) => {
                                let offset = #quoted_counter_name;
                                #quoted_counter_name += 1;
                                offset
                            }
                        }
                    });

                    quote! {{
                        #(#quoted_counters;)*
                        let mut nulls_offset = 0;

                        #data_src
                            .iter()
                            .map(|v| match v.as_deref() {
                                None => {
                                    let offset = nulls_offset;
                                    nulls_offset += 1;
                                    offset
                                }
                                #(#quoted_branches,)*
                            })
                            .collect()
                    }}
                };

                quote! {{
                    let #data_src: Vec<_> = #data_src
                        .into_iter()
                        .map(|datum| {
                            let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                            datum
                        })
                        .collect();

                    UnionArray::new(
                        #quoted_datatype,
                        #quoted_types,
                        vec![
                            NullArray::new(
                                DataType::Null,
                                #data_src.iter().filter(|v| v.is_none()).count(),
                            ).boxed(),
                            #(#quoted_field_serializers,)*
                        ],
                        Some(#quoted_offsets),
                    ).boxed()
                }}
            }
            _ => unimplemented!("{datatype:#?}"),
        }
    }
}

fn quote_arrow_field_serializer(
    objects: &Objects,
    extension_wrapper: Option<&str>,
    datatype: &DataType,
    is_nullable: bool,
    bitmap_src: &proc_macro2::Ident,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    let quoted_datatype = ArrowDataTypeTokenizer(datatype, false);
    let quoted_datatype = if let Some(ext) = extension_wrapper {
        quote!(DataType::Extension(#ext.to_owned(), Box::new(#quoted_datatype), None))
    } else {
        quote!(#quoted_datatype)
    };
    let quoted_datatype = quote! {{
        // NOTE: This is a field, it's never going to need the runtime one.
        _ = extension_wrapper;
        #quoted_datatype
            // TODO(cmc): Bring back extensions once we've fully replaced `arrow2-convert`!
            .to_logical_type().clone()
    }};

    let inner_obj = if let DataType::Extension(fqname, _, _) = datatype {
        Some(&objects[fqname])
    } else {
        None
    };
    let inner_is_arrow_transparent = inner_obj.map_or(false, |obj| obj.datatype.is_none());

    match datatype.to_logical_type() {
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Float16
        | DataType::Float32
        | DataType::Float64 => {
            // NOTE: We need values for all slots, regardless of what the bitmap says,
            // hence `unwrap_or_default`.
            let quoted_transparent_mapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                let quoted_binding = if is_tuple_struct {
                    quote!(#quoted_inner_obj_type(#quoted_data_dst))
                } else {
                    quote!(#quoted_inner_obj_type { #quoted_data_dst })
                };

                quote! {
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let #quoted_binding = datum;
                                #quoted_data_dst
                            })
                            .unwrap_or_default()
                    })
                }
            } else {
                quote! {
                    .map(|v| v.unwrap_or_default())
                }
            };

            quote! {
                PrimitiveArray::new(
                    #quoted_datatype,
                    #data_src.into_iter() #quoted_transparent_mapping .collect(),
                    #bitmap_src,
                ).boxed()
            }
        }

        DataType::Boolean => {
            quote! {
                BooleanArray::new(
                    #quoted_datatype,
                    // NOTE: We need values for all slots, regardless of what the bitmap says,
                    // hence `unwrap_or_default`.
                    #data_src.into_iter().map(|v| v.unwrap_or_default()).collect(),
                    #bitmap_src,
                ).boxed()
            }
        }

        DataType::Utf8 => {
            // NOTE: We need values for all slots, regardless of what the bitmap says,
            // hence `unwrap_or_default`.
            let (quoted_transparent_mapping, quoted_transparent_length) =
                if inner_is_arrow_transparent {
                    let inner_obj = inner_obj.as_ref().unwrap();
                    let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                    let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                    let quoted_data_dst = format_ident!(
                        "{}",
                        if is_tuple_struct {
                            "data0"
                        } else {
                            inner_obj.fields[0].name.as_str()
                        }
                    );
                    let quoted_binding = if is_tuple_struct {
                        quote!(#quoted_inner_obj_type(#quoted_data_dst))
                    } else {
                        quote!(#quoted_inner_obj_type { #quoted_data_dst })
                    };

                    (
                        quote! {
                            .flat_map(|datum| {
                                let #quoted_binding = datum;
                                #quoted_data_dst .bytes()
                            })
                        },
                        quote! {
                            .map(|datum| {
                                let #quoted_binding = datum;
                                #quoted_data_dst.len()
                            }).unwrap_or_default()
                        },
                    )
                } else {
                    (
                        quote! {
                            .flat_map(|s| s.bytes())
                        },
                        quote! {
                            .map(|datum| datum.len()).unwrap_or_default()
                        },
                    )
                };

            quote! {{
                // NOTE: Flattening to remove the guaranteed layer of nullability: we don't care
                // about it while building the backing buffer since it's all offsets driven.
                let inner_data: ::arrow2::buffer::Buffer<u8> = #data_src.iter().flatten() #quoted_transparent_mapping.collect();

                let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                    #data_src.iter().map(|opt| opt.as_ref() #quoted_transparent_length )
                ).unwrap().into();

                // Safety: we're building this from actual native strings, so no need to do the
                // whole utf8 validation _again_.
                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                unsafe { Utf8Array::<i32>::new_unchecked(#quoted_datatype, offsets, inner_data, #bitmap_src) }.boxed()
            }}
        }

        DataType::List(inner) | DataType::FixedSizeList(inner, _) => {
            let inner_datatype = inner.data_type();

            let quoted_inner_data = format_ident!("{data_src}_inner_data");
            let quoted_inner_bitmap = format_ident!("{data_src}_inner_bitmap");

            let quoted_inner = quote_arrow_field_serializer(
                objects,
                extension_wrapper,
                inner_datatype,
                inner.is_nullable,
                &quoted_inner_bitmap,
                &quoted_inner_data,
            );

            let quoted_transparent_mapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                let quoted_binding = if is_tuple_struct {
                    quote!(#quoted_inner_obj_type(#quoted_data_dst))
                } else {
                    quote!(#quoted_inner_obj_type { #quoted_data_dst })
                };

                quote! {
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let #quoted_binding = datum;
                                #quoted_data_dst
                            })
                            .unwrap_or_default()
                    })
                    // NOTE: Flattening yet again since we have to deconstruct the inner list.
                    .flatten()
                }
            } else {
                quote! {
                    .flatten()
                    // NOTE: Flattening yet again since we have to deconstruct the inner list.
                    .flatten()
                    .cloned()
                }
            };

            let quoted_create = if let DataType::List(_) = datatype {
                quote! {
                    let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                        #data_src.iter().map(|opt| opt.as_ref().map(|datum| datum.len()).unwrap_or_default())
                    ).unwrap().into();

                    ListArray::new(
                        #quoted_datatype,
                        offsets,
                        #quoted_inner,
                        #bitmap_src,
                    ).boxed()
                }
            } else {
                quote! {
                    FixedSizeListArray::new(
                        #quoted_datatype,
                        #quoted_inner,
                        #bitmap_src,
                    ).boxed()
                }
            };

            // TODO(cmc): We should be checking this, but right now we don't because we don't
            // support intra-list nullability.
            _ = is_nullable;
            quote! {{
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                let #quoted_inner_data: Vec<_> = #data_src
                    .iter()
                    #quoted_transparent_mapping
                    // NOTE: Wrapping back into an option as the recursive call will expect the
                    // guaranteed nullability layer to be present!
                    .map(Some)
                    .collect();

                // TODO(cmc): We don't support intra-list nullability in our IDL at the moment.
                let #quoted_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = None;

                #quoted_create
            }}
        }

        DataType::Struct(_) | DataType::Union(_, _, _) => {
            // NOTE: We always wrap objects with full extension metadata.
            let DataType::Extension(fqname, _, _) = datatype else { unreachable!() };
            let fqname_use = quote_fqname_as_type_path(fqname);
            let quoted_extension_wrapper =
                extension_wrapper.map_or_else(|| quote!(None::<&str>), |ext| quote!(Some(#ext)));
            quote! {{
                _ = #bitmap_src;
                _ = extension_wrapper;
                #fqname_use::try_to_arrow_opt(#data_src, #quoted_extension_wrapper)?
            }}
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}

fn quote_arrow_deserializer(
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    let datatype = &arrow_registry.get(&obj.fqname);

    let obj_fqname = obj.fqname.as_str();
    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    if is_arrow_transparent {
        // NOTE: Arrow transparent objects must have a single field, no more no less.
        // The semantic pass would have failed already if this wasn't the case.
        let obj_field = &obj.fields[0];
        let obj_field_fqname = obj_field.fqname.as_str();

        let data_src = data_src.clone();
        let data_dst = format_ident!(
            "{}",
            if is_tuple_struct {
                "data0"
            } else {
                obj_field.name.as_str()
            }
        );

        let quoted_deserializer = quote_arrow_field_deserializer(
            objects,
            &arrow_registry.get(&obj_field.fqname),
            obj_field.is_nullable,
            obj_field_fqname,
            &data_src,
        );

        let quoted_unwrap = if obj_field.is_nullable {
            quote!(.map(Ok))
        } else {
            quote!(.map(|v| v.ok_or_else(|| crate::DeserializationError::MissingData {
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            })))
        };

        let quoted_opt_map = if is_tuple_struct {
            quote!(.map(|res| res.map(|v| Some(Self(v)))))
        } else {
            quote!(.map(|res| res.map(|#data_dst| Some(Self { #data_dst }))))
        };

        quote! {
            #quoted_deserializer
            #quoted_unwrap
            #quoted_opt_map
            // NOTE: implicit Vec<Result> to Result<Vec>
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
            .map_err(|err| crate::DeserializationError::Context {
                location: #obj_field_fqname.into(),
                source: Box::new(err),
            })?
        }
    } else {
        let data_src = data_src.clone();

        // NOTE: This can only be struct or union/enum at this point.
        match datatype.to_logical_type() {
            DataType::Struct(_) => {
                let data_src_fields = format_ident!("{data_src}_fields");
                let data_src_arrays = format_ident!("{data_src}_arrays");
                let data_src_bitmap = format_ident!("{data_src}_bitmap");

                let quoted_field_deserializers = obj.fields.iter().map(|obj_field| {
                    let field_name = &obj_field.name;
                    let data_dst = format_ident!("{}", obj_field.name);

                    let quoted_deserializer = quote_arrow_field_deserializer(
                        objects,
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
                        obj_field.fqname.as_str(),
                        &data_src,
                    );

                    quote! {
                        let #data_dst = {
                            let #data_src = &**arrays_by_name[#field_name];
                             #quoted_deserializer
                        }
                    }
                });

                // NOTE: Collecting because we need it more than once.
                let quoted_field_names = obj
                    .fields
                    .iter()
                    .map(|field| format_ident!("{}", field.name))
                    .collect::<Vec<_>>();

                let quoted_unwrappings = obj.fields.iter().map(|obj_field| {
                    let obj_field_fqname = obj_field.fqname.as_str();
                    let quoted_obj_field_name = format_ident!("{}", obj_field.name);
                    if obj_field.is_nullable {
                        quote!(#quoted_obj_field_name)
                    } else {
                        quote! {
                            #quoted_obj_field_name: #quoted_obj_field_name
                                .ok_or_else(|| crate::DeserializationError::MissingData {
                                    backtrace: ::backtrace::Backtrace::new_unresolved(),
                                })
                                .map_err(|err| crate::DeserializationError::Context {
                                    location: #obj_field_fqname.into(),
                                    source: Box::new(err),
                                })?
                        }
                    }
                });

                quote! {{
                    let #data_src = #data_src
                        .as_any()
                        .downcast_ref::<::arrow2::array::StructArray>()
                        .ok_or_else(|| crate::DeserializationError::DatatypeMismatch {
                            expected: #data_src.data_type().clone(),
                            got: #data_src.data_type().clone(),
                            backtrace: ::backtrace::Backtrace::new_unresolved(),
                        })
                        .map_err(|err| crate::DeserializationError::Context {
                            location: #obj_fqname.into(),
                            source: Box::new(err),
                        })?;

                    if #data_src.is_empty() {
                        // NOTE: The outer container is empty and so we already know that the end result
                        // is also going to be an empty vec.
                        // Early out right now rather than waste time computing possibly many empty
                        // datastructures for all of our children.
                        Vec::new()
                    } else {
                        let (#data_src_fields, #data_src_arrays, #data_src_bitmap) =
                            (#data_src.fields(), #data_src.values(), #data_src.validity());

                        let is_valid = |i| #data_src_bitmap.map_or(true, |bitmap| bitmap.get_bit(i));

                        let arrays_by_name: ::std::collections::HashMap<_, _> = #data_src_fields
                            .iter()
                            .map(|field| field.name.as_str())
                            .zip(#data_src_arrays)
                            .collect();

                        #(#quoted_field_deserializers;)*

                        ::itertools::izip!(#(#quoted_field_names),*)
                            .enumerate()
                            .map(|(i, (#(#quoted_field_names),*))| is_valid(i).then(|| Ok(Self {
                                #(#quoted_unwrappings,)*
                            })).transpose())
                            // NOTE: implicit Vec<Result> to Result<Vec>
                            .collect::<crate::DeserializationResult<Vec<_>>>()
                            .map_err(|err| crate::DeserializationError::Context {
                                location: #obj_fqname.into(),
                                source: Box::new(err),
                            })?
                    }
                }}
            }

            DataType::Union(_, _, arrow2::datatypes::UnionMode::Dense) => {
                let data_src_types = format_ident!("{data_src}_types");
                let data_src_arrays = format_ident!("{data_src}_arrays");
                let data_src_offsets = format_ident!("{data_src}_offsets");

                let quoted_field_deserializers =
                    obj.fields.iter().enumerate().map(|(i, obj_field)| {
                        let data_dst = format_ident!("{}", obj_field.name.to_case(Case::Snake));

                        let quoted_deserializer = quote_arrow_field_deserializer(
                            objects,
                            &arrow_registry.get(&obj_field.fqname),
                            obj_field.is_nullable,
                            obj_field.fqname.as_str(),
                            &data_src,
                        );

                        let i = i + 1; // NOTE: +1 to account for `nulls` virtual arm

                        quote! {
                            let #data_dst = {
                                let #data_src = &*#data_src_arrays[#i];
                                 #quoted_deserializer.collect::<Vec<_>>()
                            }
                        }
                    });

                let obj_fqname = obj.fqname.as_str();
                let quoted_obj_name = format_ident!("{}", obj.name);
                let quoted_branches = obj.fields.iter().enumerate().map(|(i, obj_field)| {
                    let i = i as i8 + 1; // NOTE: +1 to account for `nulls` virtual arm

                    let obj_field_fqname = obj_field.fqname.as_str();
                    let quoted_obj_field_name =
                        format_ident!("{}", obj_field.name.to_case(Case::Snake));
                    let quoted_obj_field_type =
                        format_ident!("{}", obj_field.name.to_case(Case::UpperCamel));

                    let quoted_unwrap = if obj_field.is_nullable {
                        quote!()
                    } else {
                        quote!(.unwrap())
                    };

                    quote! {
                        #i => #quoted_obj_name::#quoted_obj_field_type(
                            #quoted_obj_field_name
                                .get(offset as usize)
                                .ok_or(crate::DeserializationError::OffsetsMismatch {
                                    bounds: (offset as usize, offset as usize),
                                    len: #quoted_obj_field_name.len(),
                                    backtrace: ::backtrace::Backtrace::new_unresolved(),
                                })
                                .map_err(|err| crate::DeserializationError::Context {
                                    location: #obj_field_fqname.into(),
                                    source: Box::new(err),
                                })?
                                .clone()
                                #quoted_unwrap
                        )
                    }
                });

                quote! {{
                    let #data_src = #data_src
                        .as_any()
                        .downcast_ref::<::arrow2::array::UnionArray>()
                        .ok_or_else(|| crate::DeserializationError::DatatypeMismatch {
                            expected: #data_src.data_type().clone(),
                            got: #data_src.data_type().clone(),
                            backtrace: ::backtrace::Backtrace::new_unresolved(),
                        })
                        .map_err(|err| crate::DeserializationError::Context {
                            location: #obj_fqname.into(),
                            source: Box::new(err),
                        })?;

                    if #data_src.is_empty() {
                        // NOTE: The outer container is empty and so we already know that the end result
                        // is also going to be an empty vec.
                        // Early out right now rather than waste time computing possibly many empty
                        // datastructures for all of our children.
                        Vec::new()
                    } else {
                        let (#data_src_types, #data_src_arrays, #data_src_offsets) =
                            // NOTE: unwrapping of offsets is safe because this is a dense union
                            (#data_src.types(), #data_src.fields(), #data_src.offsets().unwrap());

                        #(#quoted_field_deserializers;)*

                        #data_src_types
                            .iter()
                            .enumerate()
                            .map(|(i, typ)| {
                                let offset = #data_src_offsets[i];

                                if *typ == 0 {
                                    Ok(None)
                                } else {
                                    Ok(Some(match typ {
                                        #(#quoted_branches,)*
                                        _ => unreachable!(),
                                    }))
                                }
                            })
                            // NOTE: implicit Vec<Result> to Result<Vec>
                            .collect::<crate::DeserializationResult<Vec<_>>>()
                            .map_err(|err| crate::DeserializationError::Context {
                                location: #obj_fqname.into(),
                                source: Box::new(err),
                            })?
                    }
                }}
            }

            _ => unimplemented!("{datatype:#?}"),
        }
    }
}

fn quote_arrow_field_deserializer(
    objects: &Objects,
    datatype: &DataType,
    is_nullable: bool,
    obj_field_fqname: &str,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    _ = is_nullable; // not yet used, will be needed very soon

    let inner_obj = if let DataType::Extension(fqname, _, _) = datatype {
        Some(&objects[fqname])
    } else {
        None
    };
    let inner_is_arrow_transparent = inner_obj.map_or(false, |obj| obj.datatype.is_none());

    match datatype.to_logical_type() {
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::Float16
        | DataType::Float32
        | DataType::Float64
        | DataType::Boolean => {
            let quoted_transparent_unmapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                if is_tuple_struct {
                    quote!(.map(|opt| opt.map(|v| #quoted_inner_obj_type(*v))))
                } else {
                    quote!(.map(|opt| opt.map(|v| #quoted_inner_obj_type { #quoted_data_dst: *v })))
                }
            } else if *datatype.to_logical_type() == DataType::Boolean {
                quote!()
            } else {
                quote!(.map(|v| v.copied()))
            };

            let arrow_type = format!("{:?}", datatype.to_logical_type()).replace("DataType::", "");
            let arrow_type = format_ident!("{arrow_type}Array");
            quote! {
                #data_src
                    .as_any()
                    .downcast_ref::<#arrow_type>()
                    .unwrap() // safe
                    .into_iter()
                    #quoted_transparent_unmapping
            }
        }

        DataType::Utf8 => {
            let quoted_transparent_unmapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                if is_tuple_struct {
                    quote!(.map(|opt| opt.map(|v| #quoted_inner_obj_type(v.to_owned()))))
                } else {
                    quote!(.map(|opt| opt.map(|v| #quoted_inner_obj_type { #quoted_data_dst: v.to_owned() })))
                }
            } else {
                quote!(.map(|v| v.map(ToOwned::to_owned)))
            };

            quote! {
                #data_src
                    .as_any()
                    .downcast_ref::<Utf8Array<i32>>()
                    .unwrap() // safe
                    .into_iter()
                    #quoted_transparent_unmapping
            }
        }

        DataType::FixedSizeList(inner, length) => {
            let inner_datatype = inner.data_type();
            let quoted_inner = quote_arrow_field_deserializer(
                objects,
                inner_datatype,
                inner.is_nullable,
                obj_field_fqname,
                data_src,
            );

            let quoted_transparent_unmapping = if inner_is_arrow_transparent {
                let inner_obj = inner_obj.as_ref().unwrap();
                let quoted_inner_obj_type = quote_fqname_as_type_path(&inner_obj.fqname);
                let is_tuple_struct = is_tuple_struct_from_obj(inner_obj);
                let quoted_data_dst = format_ident!(
                    "{}",
                    if is_tuple_struct {
                        "data0"
                    } else {
                        inner_obj.fields[0].name.as_str()
                    }
                );
                if is_tuple_struct {
                    quote!(.map(|res| res.map(|opt| opt.map(|v| #quoted_inner_obj_type(v)))))
                } else {
                    quote!(.map(|res| res.map(|opt| opt.map(|#quoted_data_dst| #quoted_inner_obj_type { #quoted_data_dst }))))
                }
            } else {
                quote!()
            };

            quote! {{
                let #data_src = #data_src
                    .as_any()
                    .downcast_ref::<::arrow2::array::FixedSizeListArray>()
                    .unwrap(); // safe

                if #data_src.is_empty() {
                    // NOTE: The outer container is empty and so we already know that the end result
                    // is also going to be an empty vec.
                    // Early out right now rather than waste time computing possibly many empty
                    // datastructures for all of our children.
                    Vec::new()
                } else {
                    let bitmap = #data_src.validity().cloned();
                    let offsets = (0..).step_by(#length).zip((#length..).step_by(#length).take(#data_src.len()));

                    let #data_src = &**#data_src.values();

                    let data = #quoted_inner
                        .map(|v| v.ok_or_else(|| crate::DeserializationError::MissingData {
                            backtrace: ::backtrace::Backtrace::new_unresolved(),
                        }))
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<crate::DeserializationResult<Vec<_>>>()?;

                    offsets
                        .enumerate()
                        .map(move |(i, (start, end))| bitmap.as_ref().map_or(true, |bitmap| bitmap.get_bit(i)).then(|| {
                            data.get(start as usize .. end as usize)
                                .ok_or(crate::DeserializationError::OffsetsMismatch {
                                    bounds: (start as usize, end as usize),
                                    len: data.len(),
                                    backtrace: ::backtrace::Backtrace::new_unresolved(),
                                })?
                                .to_vec()
                                .try_into()
                                .map_err(|_err| crate::DeserializationError::ArrayLengthMismatch {
                                    expected: #length,
                                    got: (end - start) as usize,
                                    backtrace: ::backtrace::Backtrace::new_unresolved(),
                                })
                            }).transpose()
                        )
                        #quoted_transparent_unmapping
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                }
                .into_iter()
            }}
        }

        DataType::List(inner) => {
            let inner_datatype = inner.data_type();

            let quoted_inner = quote_arrow_field_deserializer(
                objects,
                inner_datatype,
                inner.is_nullable,
                obj_field_fqname,
                data_src,
            );

            quote! {{
                let #data_src = #data_src
                    .as_any()
                    .downcast_ref::<::arrow2::array::ListArray<i32>>()
                    .unwrap(); // safe

                if #data_src.is_empty() {
                    // NOTE: The outer container is empty and so we already know that the end result
                    // is also going to be an empty vec.
                    // Early out right now rather than waste time computing possibly many empty
                    // datastructures for all of our children.
                    Vec::new()
                } else {
                    let bitmap = #data_src.validity().cloned();
                    let offsets = {
                        let offsets = #data_src.offsets();
                        offsets.iter().copied().zip(offsets.iter().copied().skip(1))
                    };

                    let #data_src = &**#data_src.values();

                    let data = #quoted_inner
                        .map(|v| v.ok_or_else(|| crate::DeserializationError::MissingData {
                            backtrace: ::backtrace::Backtrace::new_unresolved(),
                        }))
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<crate::DeserializationResult<Vec<_>>>()?;

                    offsets
                        .enumerate()
                        .map(move |(i, (start, end))| bitmap.as_ref().map_or(true, |bitmap| bitmap.get_bit(i)).then(|| {
                                Ok(data.get(start as usize .. end as usize)
                                    .ok_or(crate::DeserializationError::OffsetsMismatch {
                                        bounds: (start as usize, end as usize),
                                        len: data.len(),
                                        backtrace: ::backtrace::Backtrace::new_unresolved(),
                                    })?
                                    .to_vec()
                                )
                            }).transpose()
                        )
                        // NOTE: implicit Vec<Result> to Result<Vec>
                        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                }
                .into_iter()
            }}
        }

        DataType::Struct(_) | DataType::Union(_, _, _) => {
            let DataType::Extension(fqname, _, _) = datatype else { unreachable!() };
            let fqname_use = quote_fqname_as_type_path(fqname);
            quote! {
                #fqname_use::try_from_arrow_opt(#data_src)
                    .map_err(|err| crate::DeserializationError::Context {
                            location: #obj_field_fqname.into(),
                            source: Box::new(err),
                        })
                    ?.into_iter()
            }
        }

        _ => unimplemented!("{datatype:#?}"),
    }
}

// --- Helpers ---

fn is_tuple_struct_from_obj(obj: &Object) -> bool {
    let is_tuple_struct = obj.kind == ObjectKind::Component
        || (obj.is_struct() && obj.try_get_attr::<String>(ATTR_RUST_TUPLE_STRUCT).is_some());

    assert!(
        !is_tuple_struct || obj.fields.len() == 1,
        "`{ATTR_RUST_TUPLE_STRUCT}` is only supported for objects with a single field, but {} has {}",
        obj.fqname,
        obj.fields.len(),
    );

    is_tuple_struct
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
