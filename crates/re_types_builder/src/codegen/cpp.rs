use std::collections::BTreeSet;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rayon::prelude::*;

use crate::{codegen::AUTOGEN_WARNING, ArrowRegistry, ObjectKind, Objects};

const NEWLINE_TOKEN: &str = "RE_TOKEN_NEWLINE";

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

        // Clean up old files:
        for entry in std::fs::read_dir(folder_path).unwrap().flatten() {
            let filepath = Utf8PathBuf::try_from(entry.path()).unwrap();
            if !filepaths.contains(&filepath) {
                std::fs::remove_file(filepath).ok();
            }
        }

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
            .replace(&format!("{NEWLINE_TOKEN:?}"), "\n"),
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
    let obj_kind_ident = format_ident!("{}", obj.kind.plural_snake_case());

    let pascal_case_name = &obj.name;
    let pascal_case_ident = format_ident!("{pascal_case_name}");
    let snake_case_name = obj.snake_case_name();

    let hash = quote! { # };
    let pragma_once = pragma_once();
    let header_file_name = format!("{snake_case_name}.hpp");

    let hpp = quote! {
        #pragma_once
        namespace rr {
            namespace #obj_kind_ident {
                struct #pascal_case_ident { };
            }
        }
    };
    let cpp = quote! { #hash include #header_file_name };

    (hpp, cpp)
}

fn pragma_once() -> TokenStream {
    let hash = quote! { # };
    quote! {
        #hash pragma once #NEWLINE_TOKEN #NEWLINE_TOKEN
    }
}
