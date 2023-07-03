//! Implements the Rust codegen pass.

use anyhow::Context as _;
use arrow2::datatypes::DataType;
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

// TODO(cmc): it'd be nice to be able to generate vanilla comments (as opposed to doc-comments)
// once again at some point (`TokenStream` strips them)... nothing too urgent though.

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
            objects,
            &objects.ordered_objects(ObjectKind::Datatype.into()),
        ));

        let components_path = self.crate_path.join("src/components");
        std::fs::create_dir_all(&components_path)
            .with_context(|| format!("{components_path:?}"))
            .unwrap();
        filepaths.extend(create_files(
            components_path,
            arrow_registry,
            objects,
            &objects.ordered_objects(ObjectKind::Component.into()),
        ));

        let archetypes_path = self.crate_path.join("src/archetypes");
        std::fs::create_dir_all(&archetypes_path)
            .with_context(|| format!("{archetypes_path:?}"))
            .unwrap();
        filepaths.extend(create_files(
            archetypes_path,
            arrow_registry,
            objects,
            &objects.ordered_objects(ObjectKind::Archetype.into()),
        ));

        filepaths
    }
}

// --- File management ---

fn create_files(
    out_path: impl AsRef<Path>,
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    objs: &[&Object],
) -> Vec<PathBuf> {
    let out_path = out_path.as_ref();

    let mut filepaths = Vec::new();

    let mut files = HashMap::<PathBuf, Vec<QuotedObject>>::new();
    for obj in objs {
        let obj = if obj.is_struct() {
            QuotedObject::from_struct(arrow_registry, objects, obj)
        } else {
            QuotedObject::from_union(arrow_registry, objects, obj)
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
        code.push_text("#![allow(trivial_numeric_casts)]", 2, 0);
        code.push_text("#![allow(unused_parens)]", 2, 0);
        code.push_text("#![allow(clippy::clone_on_copy)]", 2, 0);
        code.push_text("#![allow(clippy::map_flatten)]", 2, 0);
        code.push_text("#![allow(clippy::needless_question_mark)]", 2, 0);
        code.push_text("#![allow(clippy::too_many_arguments)]", 2, 0);
        code.push_text("#![allow(clippy::unnecessary_cast)]", 2, 0);

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
    fn from_struct(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> Self {
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

        let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, objects, obj);

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

    fn from_union(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> Self {
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
                is_nullable: _,
                is_deprecated: _,
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

        let quoted_trait_impls = quote_trait_impls_from_obj(arrow_registry, objects, obj);

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

fn quote_trait_impls_from_obj(
    arrow_registry: &ArrowRegistry,
    objects: &Objects,
    obj: &Object,
) -> TokenStream {
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

            let quoted_serializer =
                quote_arrow_serializer(arrow_registry, obj, &format_ident!("data"));
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

                impl crate::#kind for #name {
                    #[inline]
                    fn name() -> crate::#kind_name {
                        crate::#kind_name::Borrowed(#legacy_fqname)
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
                        use crate::{Component as _, Datatype as _};
                        Ok(#quoted_serializer)
                    }

                    // NOTE: Don't inline this, this gets _huge_.
                    #[allow(unused_imports, clippy::wildcard_imports)]
                    fn try_from_arrow_opt(data: &dyn ::arrow2::array::Array) -> crate::DeserializationResult<Vec<Option<Self>>>
                    where
                        Self: Sized {
                        use ::arrow2::{datatypes::*, array::*};
                        use crate::{Component as _, Datatype as _};
                        Ok(#quoted_deserializer)
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

            let quoted_field_names = obj
                .fields
                .iter()
                .map(|field| format_ident!("{}", field.name))
                .collect::<Vec<_>>();

            let all_serializers = {
                obj.fields.iter().map(|obj_field| {
                    let field_name_str = &obj_field.name;
                    let field_name = format_ident!("{}", obj_field.name);

                    let is_plural = obj_field.typ.is_plural();
                    let is_nullable = obj_field.is_nullable;

                    // NOTE: unwrapping is safe since the field must point to a component.
                    let component = obj_field.typ.fqname().unwrap();
                    let component = format_ident!("{}", component.rsplit_once('.').unwrap().1);
                    let component = quote!(crate::components::#component);

                    let fqname = obj_field.typ.fqname().unwrap();
                    let legacy_fqname = objects
                        .get(fqname)
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
                            .transpose()?
                        },
                        (true, false) => quote! {
                            Some({
                                let array = <#component>::try_to_arrow(self.#field_name.iter(), None);
                                #extract_datatype_and_return
                            })
                            .transpose()?
                        },
                        (false, true) => quote! {
                             self.#field_name.as_ref().map(|single| {
                                let array = <#component>::try_to_arrow([single], None);
                                #extract_datatype_and_return
                            })
                            .transpose()?
                        },
                        (false, false) => quote! {
                            Some({
                                let array = <#component>::try_to_arrow([&self.#field_name], None);
                                #extract_datatype_and_return
                            })
                            .transpose()?
                        },
                    }
                })
            };

            let all_deserializers = {
                obj.fields.iter().map(|obj_field| {
                    let field_name_str = &obj_field.name;
                    let field_name = format_ident!("{}", obj_field.name);

                    let is_plural = obj_field.typ.is_plural();
                    let is_nullable = obj_field.is_nullable;

                    // NOTE: unwrapping is safe since the field must point to a component.
                    let component = obj_field.typ.fqname().unwrap();
                    let component = format_ident!("{}", component.rsplit_once('.').unwrap().1);
                    let component = quote!(crate::components::#component);

                    let quoted_collection = if is_plural {
                        quote! {
                            .into_iter()
                            .map(|v| v .ok_or_else(|| crate::DeserializationError::MissingData {
                                // TODO(cmc): gotta improve this
                                datatype: ::arrow2::datatypes::DataType::Null,
                            }))
                            .collect::<crate::DeserializationResult<Vec<_>>>()?
                        }
                    } else {
                        quote! {
                            .into_iter()
                            .next()
                            .flatten()
                            .ok_or_else(|| crate::DeserializationError::MissingData {
                                // TODO(cmc): gotta improve this
                                datatype: ::arrow2::datatypes::DataType::Null,
                            })?
                        }
                    };

                    let quoted_deser = if is_nullable {
                        quote! {
                            if let Some(array) = arrays_by_name.get(#field_name_str) {
                                Some(<#component>::try_from_arrow_opt(&**array)? #quoted_collection)
                            } else {
                                None
                            }
                        }
                    } else {
                        quote! {{
                            let array = arrays_by_name
                                .get(#field_name_str)
                                .ok_or_else(|| crate::DeserializationError::MissingData {
                                    // TODO(cmc): gotta improve this
                                    datatype: ::arrow2::datatypes::DataType::Null,
                                })?;
                            <#component>::try_from_arrow_opt(&**array)? #quoted_collection
                        }}
                    };

                    quote!(let #field_name = #quoted_deser;)
                })
            };

            quote! {
                impl #name {
                    pub const REQUIRED_COMPONENTS: [crate::ComponentName; #num_required] = [#required];

                    pub const RECOMMENDED_COMPONENTS: [crate::ComponentName; #num_recommended] = [#recommended];

                    pub const OPTIONAL_COMPONENTS: [crate::ComponentName; #num_optional] = [#optional];

                    pub const ALL_COMPONENTS: [crate::ComponentName; #num_all] = [#required #recommended #optional];
                }

                impl crate::Archetype for #name {
                    #[inline]
                    fn name() -> crate::ArchetypeName {
                        crate::ArchetypeName::Borrowed(#fqname)
                    }

                    #[inline]
                    fn required_components() -> Vec<crate::ComponentName> {
                        Self::REQUIRED_COMPONENTS.to_vec()
                    }

                    #[inline]
                    fn recommended_components() -> Vec<crate::ComponentName> {
                        Self::RECOMMENDED_COMPONENTS.to_vec()
                    }

                    #[inline]
                    fn optional_components() -> Vec<crate::ComponentName> {
                        Self::OPTIONAL_COMPONENTS.to_vec()
                    }

                    #[inline]
                    fn try_to_arrow(
                        &self,
                    ) -> crate::SerializationResult<Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>> {
                        use crate::Component as _;
                        Ok([ #({ #all_serializers },)* ].into_iter().flatten().collect())
                    }

                    #[inline]
                    fn try_from_arrow(
                        data: impl IntoIterator<Item = (::arrow2::datatypes::Field, Box<dyn::arrow2::array::Array>)>,
                    ) -> crate::DeserializationResult<Self> {
                        use crate::Component as _;

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

struct ArrowDataTypeTokenizer<'a>(&'a ::arrow2::datatypes::DataType);

impl quote::ToTokens for ArrowDataTypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use arrow2::datatypes::UnionMode;
        // TODO(cmc): Bring back extensions once we've fully replaced `arrow2-convert`!
        match self.0.to_logical_type() {
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
    let fqname = fqname
        .as_ref()
        .replace(".testing", "")
        .replace('.', "::")
        .replace("rerun", "crate");
    let expr: syn::TypePath = syn::parse_str(&fqname).unwrap();
    quote!(#expr)
}

// --- Serialization ---

fn quote_arrow_serializer(
    arrow_registry: &ArrowRegistry,
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
            Some(obj.fqname.as_str()),
            &arrow_registry.get(&obj_field.fqname),
            obj_field.is_nullable,
            &bitmap_dst,
            &quoted_data_dst,
        );

        let quoted_bitmap = quoted_bitmap(bitmap_dst);

        let quoted_flatten = quoted_flatten(obj_field.is_nullable);

        quote! { {
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
        } }
    } else {
        let data_src = data_src.clone();

        // NOTE: This can only be struct or union/enum at this point.
        match datatype.to_logical_type() {
            DataType::Struct(_) => {
                let quoted_field_serializers = obj.fields.iter().map(|obj_field| {
                    let data_dst = format_ident!("{}", obj_field.name);
                    let bitmap_dst = format_ident!("{data_dst}_bitmap");

                    let quoted_serializer = quote_arrow_field_serializer(
                        None,
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
                        &bitmap_dst,
                        &data_dst,
                    );

                    let quoted_flatten = quoted_flatten(obj_field.is_nullable);

                    let quoted_bitmap = quoted_bitmap(bitmap_dst);

                    quote! { {
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
                    } }
                });

                let quoted_bitmap = quoted_bitmap(format_ident!("bitmap"));

                quote! { {
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
                } }
            }
            _ => unimplemented!("{datatype:#?}"), // NOLINT
        }
    }
}

fn quote_arrow_field_serializer(
    extension_wrapper: Option<&str>,
    datatype: &DataType,
    is_nullable: bool,
    bitmap_src: &proc_macro2::Ident,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    let quoted_datatype = ArrowDataTypeTokenizer(datatype);
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
            quote! {
                PrimitiveArray::new(
                    #quoted_datatype,
                    // NOTE: We need values for all slots, regardless of what the bitmap says,
                    // hence `unwrap_or_default`.
                    #data_src.into_iter().map(|v| v.unwrap_or_default()).collect(),
                    #bitmap_src,
                ).boxed()
            }
        }

        DataType::Utf8 => {
            quote! { {
                // NOTE: Flattening to remove the guaranteed layer of nullability: we don't care
                // about it while building the backing buffer since it's all offsets driven.
                let inner_data: ::arrow2::buffer::Buffer<u8> = #data_src.iter().flatten().flat_map(|s| s.bytes()).collect();

                let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                    #data_src.iter().map(|opt| opt.as_ref().map(|datum| datum.len()).unwrap_or_default())
                ).unwrap().into();

                // Safety: we're building this from actual native strings, so no need to do the
                // whole utf8 validation _again_.
                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                unsafe { Utf8Array::<i32>::new_unchecked(#quoted_datatype, offsets, inner_data, #bitmap_src) }.boxed()
            } }
        }

        DataType::List(inner) | DataType::FixedSizeList(inner, _) => {
            let inner_datatype = inner.data_type();

            let quoted_inner_data = format_ident!("{data_src}_inner_data");
            let quoted_inner_bitmap = format_ident!("{data_src}_inner_bitmap");

            let quoted_inner = quote_arrow_field_serializer(
                extension_wrapper,
                inner_datatype,
                inner.is_nullable,
                &quoted_inner_bitmap,
                &quoted_inner_data,
            );

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
            quote! { {
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};

                let #quoted_inner_data: Vec<_> = #data_src
                    .iter()
                    // NOTE: Flattening to remove the guaranteed layer of nullability, we don't care
                    // about it while building the backing buffer.
                    .flatten()
                    // NOTE: Flattening yet again since we have to deconstruct the inner list.
                    .flatten()
                    .map(ToOwned::to_owned)
                    // NOTE: Wrapping back into an option as the recursive call will expect the
                    // guaranteed nullability layer to be present!
                    .map(Some)
                    .collect();

                let #quoted_inner_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                    let any_nones = #quoted_inner_data.iter().any(|v| v.is_none());
                    any_nones.then(|| #quoted_inner_data.iter().map(|v| v.is_some()).collect())
                };

                #quoted_create
            } }
        }

        DataType::Struct(_) => {
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

        _ => unimplemented!("{datatype:#?}"), // NOLINT
    }
}

fn quote_arrow_deserializer(
    arrow_registry: &ArrowRegistry,
    _objects: &Objects,
    obj: &Object,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    let datatype = &arrow_registry.get(&obj.fqname);

    let is_arrow_transparent = obj.datatype.is_none();
    let is_tuple_struct = is_tuple_struct_from_obj(obj);

    if is_arrow_transparent {
        // NOTE: Arrow transparent objects must have a single field, no more no less.
        // The semantic pass would have failed already if this wasn't the case.
        let obj_field = &obj.fields[0];

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
            &arrow_registry.get(&obj_field.fqname),
            obj_field.is_nullable,
            &data_src,
        );

        let quoted_unwrap = if obj_field.is_nullable {
            quote!(.map(Ok))
        } else {
            quote! {
                .map(|v| v.ok_or_else(|| crate::DeserializationError::MissingData {
                    datatype: #data_src.data_type().clone(),
                }))
            }
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
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
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
                        &arrow_registry.get(&obj_field.fqname),
                        obj_field.is_nullable,
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
                    let quoted_obj_field_name = format_ident!("{}", obj_field.name);
                    if obj_field.is_nullable {
                        quote!(#quoted_obj_field_name)
                    } else {
                        quote! {
                            #quoted_obj_field_name: #quoted_obj_field_name
                                .ok_or_else(|| crate::DeserializationError::MissingData {
                                    datatype: #data_src.data_type().clone(),
                                })?
                        }
                    }
                });

                quote! { {
                    let #data_src = #data_src
                        .as_any()
                        .downcast_ref::<::arrow2::array::StructArray>()
                        .ok_or_else(|| crate::DeserializationError::SchemaMismatch {
                            expected: #data_src.data_type().clone(),
                            got: #data_src.data_type().clone(),
                        })?;

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
                        .collect::<crate::DeserializationResult<Vec<_>>>()?
                } }
            }
            _ => unimplemented!("{datatype:#?}"), // NOLINT
        }
    }
}

fn quote_arrow_field_deserializer(
    datatype: &DataType,
    is_nullable: bool,
    data_src: &proc_macro2::Ident,
) -> TokenStream {
    _ = is_nullable; // not yet used, will be needed very soon
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
            let arrow_type = format!("{:?}", datatype.to_logical_type()).replace("DataType::", "");
            let arrow_type = format_ident!("{arrow_type}Array");
            quote! {
                #data_src
                    .as_any()
                    .downcast_ref::<#arrow_type>()
                    .unwrap() // safe
                    .into_iter()
                    .map(|v| v.copied())
            }
        }

        DataType::Utf8 => {
            quote! {
                #data_src
                    .as_any()
                    .downcast_ref::<Utf8Array<i32>>()
                    .unwrap() // safe
                    .into_iter()
                    .map(|v| v.map(ToOwned::to_owned))
            }
        }

        DataType::FixedSizeList(inner, length) => {
            let inner_datatype = inner.data_type();
            let quoted_inner_datatype = ArrowDataTypeTokenizer(inner_datatype);

            let quoted_inner =
                quote_arrow_field_deserializer(inner_datatype, inner.is_nullable, data_src);

            quote! { {
                let datatype = #data_src.data_type();
                let #data_src = #data_src
                    .as_any()
                    .downcast_ref::<::arrow2::array::ListArray<i32>>()
                    .unwrap(); // safe

                let bitmap = #data_src.validity().cloned();
                let offsets = (0..).step_by(#length).zip((#length..).step_by(#length));

                let #data_src = &**#data_src.values();

                let data = #quoted_inner
                    .map(|v| v.ok_or_else(|| crate::DeserializationError::MissingData {
                        datatype: #quoted_inner_datatype,
                    }))
                    // NOTE: implicit Vec<Result> to Result<Vec>
                    .collect::<crate::DeserializationResult<Vec<_>>>()?;

                offsets
                    .enumerate()
                    .map(move |(i, (start, end))| bitmap.as_ref().map_or(true, |bitmap| bitmap.get_bit(i)).then(|| {
                        data.get(start as usize .. end as usize)
                            .ok_or_else(|| crate::DeserializationError::OffsetsMismatch {
                                bounds: (start as usize, end as usize),
                                len: data.len(),
                                datatype: datatype.clone(),
                            })?
                            .to_vec()
                            .try_into()
                            .map_err(|_err| crate::DeserializationError::ArrayLengthMismatch {
                                expected: #length,
                                got: (end - start) as usize,
                                datatype: datatype.clone(),
                            })
                        }).transpose()
                    )
                    // NOTE: implicit Vec<Result> to Result<Vec>
                    .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                    .into_iter()
            } }
        }

        DataType::List(inner) => {
            let inner_datatype = inner.data_type();
            let quoted_inner_datatype = ArrowDataTypeTokenizer(inner_datatype);

            let quoted_inner =
                quote_arrow_field_deserializer(inner_datatype, inner.is_nullable, data_src);

            quote! { {
                let datatype = #data_src.data_type();
                let #data_src = #data_src
                    .as_any()
                    .downcast_ref::<::arrow2::array::ListArray<i32>>()
                    .unwrap(); // safe

                let bitmap = #data_src.validity().cloned();
                let offsets = {
                    let offsets = #data_src.offsets();
                    offsets.iter().copied().zip(offsets.iter().copied().skip(1))
                };

                let #data_src = &**#data_src.values();

                let data = #quoted_inner
                    .map(|v| v.ok_or_else(|| crate::DeserializationError::MissingData {
                        datatype: #quoted_inner_datatype,
                    }))
                    // NOTE: implicit Vec<Result> to Result<Vec>
                    .collect::<crate::DeserializationResult<Vec<_>>>()?;

                offsets
                    .enumerate()
                    .map(move |(i, (start, end))| bitmap.as_ref().map_or(true, |bitmap| bitmap.get_bit(i)).then(|| {
                            Ok(data.get(start as usize .. end as usize)
                                .ok_or_else(|| crate::DeserializationError::OffsetsMismatch {
                                    bounds: (start as usize, end as usize),
                                    len: data.len(),
                                    datatype: datatype.clone(),
                                })?
                                .to_vec()
                            )
                        }).transpose()
                    )
                    // NOTE: implicit Vec<Result> to Result<Vec>
                    .collect::<crate::DeserializationResult<Vec<Option<_>>>>()?
                    .into_iter()
            } }
        }

        DataType::Struct(_) => {
            let DataType::Extension(fqname, _, _) = datatype else { unreachable!() };
            let fqname_use = quote_fqname_as_type_path(fqname);
            quote!(#fqname_use::try_from_arrow_opt(#data_src)?.into_iter())
        }

        _ => unimplemented!("{datatype:#?}"), // NOLINT
    }
}

// --- Helpers ---

fn is_tuple_struct_from_obj(obj: &Object) -> bool {
    let is_tuple_struct =
        obj.is_struct() && obj.try_get_attr::<String>(ATTR_RUST_TUPLE_STRUCT).is_some();

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
