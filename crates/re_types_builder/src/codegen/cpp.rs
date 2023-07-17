use std::collections::BTreeSet;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rayon::prelude::*;

use crate::{
    codegen::AUTOGEN_WARNING, ArrowRegistry, ElementType, ObjectField, ObjectKind, Objects, Type,
};

const NEWLINE_TOKEN: &str = "RE_TOKEN_NEWLINE";
const TODO_TOKEN: &str = "RE_TOKEN_TODO";

pub struct CppCodeGenerator {
    output_path: Utf8PathBuf,
}

impl CppCodeGenerator {
    pub fn new(output_path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            output_path: output_path.into(),
        }
    }

    fn generate_folder(
        &self,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
        object_kind: ObjectKind,
        folder_name: &str,
    ) -> BTreeSet<Utf8PathBuf> {
        let folder_path = self.output_path.join(folder_name);
        std::fs::create_dir_all(&folder_path)
            .with_context(|| format!("{folder_path:?}"))
            .unwrap();

        let mut filepaths = BTreeSet::default();

        // Generate folder contents:
        let ordered_objects = objects.ordered_objects(object_kind.into());
        for &obj in &ordered_objects {
            let filename = obj.snake_case_name();
            let (hpp, cpp) = generate_hpp_cpp(objects, arrow_registry, obj);
            for (extension, tokens) in [("hpp", hpp), ("cpp", cpp)] {
                let string = string_from_token_stream(&tokens, obj.relative_filepath());
                let filepath = folder_path.join(format!("{filename}.{extension}"));
                write_file(&filepath, string);
                let inserted = filepaths.insert(filepath);
                assert!(
                    inserted,
                    "Multiple objects with the same name: {:?}",
                    obj.name
                );
            }
        }

        {
            // Generate module file that includes all the headers:
            let hash = quote! { # };
            let pragma_once = pragma_once();
            let header_file_names = ordered_objects
                .iter()
                .map(|obj| format!("{folder_name}/{}.hpp", obj.snake_case_name()));
            let tokens = quote! {
                #pragma_once
                #(#hash include #header_file_names "RE_TOKEN_NEWLINE")*
            };
            let filepath = folder_path
                .parent()
                .unwrap()
                .join(format!("{folder_name}.hpp"));
            let string = string_from_token_stream(&tokens, None);
            write_file(&filepath, string);
            filepaths.insert(filepath);
        }

        super::common::remove_old_files_from_folder(folder_path, &filepaths);

        filepaths
    }
}

impl crate::CodeGenerator for CppCodeGenerator {
    fn generate(
        &mut self,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
    ) -> BTreeSet<Utf8PathBuf> {
        ObjectKind::ALL
            .par_iter()
            .map(|object_kind| {
                let folder_name = object_kind.plural_snake_case();
                self.generate_folder(objects, arrow_registry, *object_kind, folder_name)
            })
            .flatten()
            .collect()
    }
}

fn string_from_token_stream(token_stream: &TokenStream, source_path: Option<&Utf8Path>) -> String {
    let mut code = String::new();
    code.push_str(&format!("// {AUTOGEN_WARNING}\n"));
    if let Some(source_path) = source_path {
        code.push_str(&format!("// Based on {source_path:?}\n"));
    }

    code.push('\n');
    code.push_str(
        &token_stream
            .to_string()
            .replace(&format!("{NEWLINE_TOKEN:?}"), "\n")
            .replace(
                &format!("{TODO_TOKEN:?}"),
                "\n// TODO(#2647): code-gen for C++\n",
            )
            .replace("< ", "<")
            .replace(" >", ">")
            .replace(" ::", "::")
            .replace("crate::", "rr::"),
    );
    code.push('\n');

    // clang_format has a bit of an ugly API: https://github.com/KDAB/clang-format-rs/issues/3
    clang_format::CLANG_FORMAT_STYLE
        .set(clang_format::ClangFormatStyle::File)
        .ok();
    code = clang_format::clang_format(&code).expect("Failed to run clang-format");

    code
}

fn write_file(filepath: &Utf8PathBuf, code: String) {
    if let Ok(existing) = std::fs::read_to_string(filepath) {
        if existing == code {
            // Don't touch the timestamp unnecessarily
            return;
        }
    }

    std::fs::write(filepath, code)
        .with_context(|| format!("{filepath}"))
        .unwrap();
}

fn generate_hpp_cpp(
    _objects: &Objects,
    _arrow_registry: &ArrowRegistry,
    obj: &crate::Object,
) -> (TokenStream, TokenStream) {
    let QuotedObject { hpp, cpp } = QuotedObject::new(obj);
    let snake_case_name = obj.snake_case_name();
    let hash = quote! { # };
    let pragma_once = pragma_once();
    let header_file_name = format!("{snake_case_name}.hpp");

    let hpp = quote! {
        #pragma_once
        #hpp
    };
    let cpp = quote! {
        #hash include #header_file_name #NEWLINE_TOKEN #NEWLINE_TOKEN
        #cpp
    };

    (hpp, cpp)
}

fn pragma_once() -> TokenStream {
    let hash = quote! { # };
    quote! {
        #hash pragma once #NEWLINE_TOKEN #NEWLINE_TOKEN
    }
}

struct QuotedObject {
    hpp: TokenStream,
    cpp: TokenStream,
}

impl QuotedObject {
    pub fn new(obj: &crate::Object) -> Self {
        match obj.specifics {
            crate::ObjectSpecifics::Struct => Self::from_struct(obj),
            crate::ObjectSpecifics::Union { .. } => Self::from_union(obj),
        }
    }

    fn from_struct(obj: &crate::Object) -> QuotedObject {
        let obj_kind_ident = format_ident!("{}", obj.kind.plural_snake_case());
        let pascal_case_name = &obj.name;
        let pascal_case_ident = format_ident!("{pascal_case_name}");
        let hash = quote! { # };

        let field_declarations = obj
            .fields
            .iter()
            .map(|obj_field| quote_declaration(obj_field, false).0);

        let hpp = quote! {
            #hash include <cstdint> #NEWLINE_TOKEN
            #hash include <optional> #NEWLINE_TOKEN
            #hash include <vector> #NEWLINE_TOKEN
            #NEWLINE_TOKEN

            namespace rr {
                namespace #obj_kind_ident {
                    struct #pascal_case_ident {
                        #(#field_declarations;)*
                    };
                }
            }
        };
        let cpp = quote! {
            #TODO_TOKEN
        };

        Self { hpp, cpp }
    }

    fn from_union(obj: &crate::Object) -> QuotedObject {
        let obj_kind_ident = format_ident!("{}", obj.kind.plural_snake_case());

        let hpp = quote! {
            namespace rr {
                namespace #obj_kind_ident {
                    #TODO_TOKEN
                }
            }
        };
        let cpp = quote! {
            #TODO_TOKEN
        };

        Self { hpp, cpp }
    }
}

/// Returns type name as string and whether it was force unwrapped.
///
/// Specifying `unwrap = true` will unwrap the final type before returning it, e.g. `Vec<String>`
/// becomes just `String`.
/// The returned boolean indicates whether there was anything to unwrap at all.
fn quote_declaration(obj_field: &ObjectField, unwrap: bool) -> (TokenStream, bool) {
    let obj_field_type = TypeTokenizer { obj_field, unwrap };
    let unwrapped = unwrap && matches!(obj_field.typ, Type::Array { .. } | Type::Vector { .. });
    (quote!(#obj_field_type), unwrapped)
}

struct TypeTokenizer<'a> {
    obj_field: &'a ObjectField,
    unwrap: bool,
}

impl quote::ToTokens for TypeTokenizer<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { obj_field, unwrap } = self;

        let name = format_ident!("{}", obj_field.name);
        if obj_field.is_nullable {
            match &obj_field.typ {
                Type::UInt8 => quote! { std::optional<uint8_t> #name },
                Type::UInt16 => quote! { std::optional<uint16_t> #name },
                Type::UInt32 => quote! { std::optional<uint32_t> #name },
                Type::UInt64 => quote! { std::optional<uint64_t> #name },
                Type::Int8 => quote! { std::optional<int8_t> #name },
                Type::Int16 => quote! { std::optional<int16_t> #name },
                Type::Int32 => quote! { std::optional<int32_t> #name },
                Type::Int64 => quote! { std::optional<int64_t> #name },
                Type::Bool => quote! { std::optional<bool> #name },
                Type::Float16 => unimplemented!("{:#?}", obj_field.typ), // NOLINT
                Type::Float32 => quote! { std::optional<float> #name },
                Type::Float64 => quote! { std::optional<double> #name },
                Type::String => quote! { std::optional<std::string> #name },
                Type::Array { .. } => unimplemented!("{:#?}", obj_field.typ), // NOLINT
                Type::Vector { elem_type } => {
                    let elem_type = quote_element_type(elem_type);
                    if *unwrap {
                        quote! { std::optional<#elem_type> #name }
                    } else {
                        quote! { std::optional<std::vector<#elem_type>> #name }
                    }
                }
                Type::Object(fqname) => {
                    let type_name = quote_fqname_as_type_path(fqname);
                    quote! { std::optional<#type_name> #name }
                }
            }
        } else {
            match &obj_field.typ {
                Type::UInt8 => quote! { uint8_t #name },
                Type::UInt16 => quote! { uint16_t #name },
                Type::UInt32 => quote! { uint32_t #name },
                Type::UInt64 => quote! { uint64_t #name },
                Type::Int8 => quote! { int8_t #name },
                Type::Int16 => quote! { int16_t #name },
                Type::Int32 => quote! { int32_t #name },
                Type::Int64 => quote! { int64_t #name },
                Type::Bool => quote! { bool #name },
                Type::Float16 => unimplemented!("{:#?}", obj_field.typ), // NOLINT
                Type::Float32 => quote! { float #name },
                Type::Float64 => quote! { double #name },
                Type::String => quote! { std::string #name },
                Type::Array { elem_type, length } => {
                    let elem_type = quote_element_type(elem_type);
                    let length = proc_macro2::Literal::usize_unsuffixed(*length);
                    if *unwrap {
                        quote! { #elem_type #name }
                    } else {
                        quote! { #elem_type #name[#length] }
                    }
                }
                Type::Vector { elem_type } => {
                    let elem_type = quote_element_type(elem_type);
                    if *unwrap {
                        quote! { #elem_type #name }
                    } else {
                        quote! { std::vector<#elem_type> #name }
                    }
                }
                Type::Object(fqname) => {
                    let type_name = quote_fqname_as_type_path(fqname);
                    quote! { #type_name #name }
                }
            }
        }
        .to_tokens(tokens);
    }
}

fn quote_element_type(typ: &ElementType) -> TokenStream {
    match typ {
        ElementType::UInt8 => quote! { uint8_t },
        ElementType::UInt16 => quote! { uint16_t },
        ElementType::UInt32 => quote! { uint32_t },
        ElementType::UInt64 => quote! { uint64_t },
        ElementType::Int8 => quote! { int8_t },
        ElementType::Int16 => quote! { int16_t },
        ElementType::Int32 => quote! { int32_t },
        ElementType::Int64 => quote! { int64_t },
        ElementType::Bool => quote! { bool },
        ElementType::Float16 => unimplemented!("{typ:#?}"), // NOLINT
        ElementType::Float32 => quote! { float },
        ElementType::Float64 => quote! { double },
        ElementType::String => quote! { std::string },
        ElementType::Object(fqname) => quote_fqname_as_type_path(fqname),
    }
}

fn quote_fqname_as_type_path(fqname: &str) -> TokenStream {
    let fqname = fqname
        .replace(".testing", "")
        .replace('.', "::")
        .replace("crate", "rr")
        .replace("rerun", "rr");
    let expr: syn::TypePath = syn::parse_str(&fqname).unwrap();
    quote!(#expr)
}
