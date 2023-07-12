use std::collections::BTreeSet;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::{codegen::AUTOGEN_WARNING, ArrowRegistry, Object, ObjectKind, Objects};

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
        &mut self,
        objects: &Objects,
        arrow_registry: &ArrowRegistry,
        object_kind: ObjectKind,
        folder_name: &str,
    ) -> BTreeSet<Utf8PathBuf> {
        let mut filepaths = BTreeSet::default();

        let folder_path = self.output_path.join(folder_name);
        std::fs::create_dir_all(&folder_path)
            .with_context(|| format!("{folder_path:?}"))
            .unwrap();
        for obj in objects.ordered_objects(object_kind.into()) {
            let filename = obj.snake_case_name();
            let (hpp, cpp) = generate_hpp_cpp(objects, arrow_registry, obj);
            for (extension, tokens) in [("hpp", hpp), ("cpp", cpp)] {
                let string = string_from_token_stream(obj, &tokens);
                let filepath = folder_path.join(format!("{filename}.{extension}"));
                write_file(&filepath, string);
                filepaths.insert(filepath);
            }
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
        let mut filepaths = BTreeSet::new();

        for object_kind in ObjectKind::ALL {
            let folder_name = object_kind.plural_snake_case();
            filepaths.extend(self.generate_folder(
                objects,
                arrow_registry,
                object_kind,
                folder_name,
            ));
        }

        filepaths
    }
}

fn string_from_token_stream(obj: &Object, token_stream: &TokenStream) -> String {
    let mut code = String::new();
    code.push_str(&format!("// {AUTOGEN_WARNING}\n"));
    if let Some(relative_path) = obj.relative_filepath() {
        code.push_str(&format!("// Based on {relative_path:?}"));
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
    let header_file_name = format!("{snake_case_name}.hpp");

    let hpp = quote! {
        #hash pragma once #NEWLINE_TOKEN #NEWLINE_TOKEN

        namespace rr {
            namespace #obj_kind_ident {
                struct #pascal_case_ident { };
            }
        }
    };
    let cpp = quote! { #hash include #header_file_name };

    (hpp, cpp)
}
