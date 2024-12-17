mod array_builder;
mod forward_decl;
mod includes;
mod method;

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use rayon::prelude::*;

use crate::{
    codegen::{autogen_warning, common::collect_snippets_for_api_docs},
    format_path,
    objects::ObjectClass,
    ArrowRegistry, Docs, ElementType, GeneratedFiles, Object, ObjectField, ObjectKind, Objects,
    Reporter, Type, ATTR_CPP_NO_FIELD_CTORS, ATTR_CPP_RENAME_FIELD,
    ATTR_RERUN_LOG_MISSING_AS_EMPTY,
};

use self::array_builder::{arrow_array_builder_type, arrow_array_builder_type_object};
use self::forward_decl::{ForwardDecl, ForwardDecls};
use self::includes::Includes;
use self::method::{Method, MethodDeclaration};

use super::{common::ExampleInfo, Target};

type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;

trait CppObjectExtensions {
    fn namespace_ident(&self) -> Ident;
    fn ident(&self) -> Ident;
}

impl CppObjectExtensions for Object {
    fn namespace_ident(&self) -> Ident {
        format_ident!("{}", self.kind.plural_snake_case())
    }

    fn ident(&self) -> Ident {
        format_ident!("{}", self.name) // The PascalCase name of the object type.
    }
}

// Special strings we insert as tokens, then search-and-replace later.
// This is so that we can insert comments and whitespace into the generated code.
// `TokenStream` ignores whitespace (including comments), but we can insert "quoted strings",
// so that is what we do.
const NEWLINE_TOKEN: &str = "NEWLINE_TOKEN";
const NORMAL_COMMENT_PREFIX_TOKEN: &str = "NORMAL_COMMENT_PREFIX_TOKEN";
const NORMAL_COMMENT_SUFFIX_TOKEN: &str = "NORMAL_COMMENT_SUFFIX_TOKEN";
const DOC_COMMENT_PREFIX_TOKEN: &str = "DOC_COMMENT_PREFIX_TOKEN";
const DOC_COMMENT_SUFFIX_TOKEN: &str = "DOC_COMMENT_SUFFIX_TOKEN";
const ANGLE_BRACKET_LEFT_TOKEN: &str = "SYS_INCLUDE_PATH_PREFIX_TOKEN";
const ANGLE_BRACKET_RIGHT_TOKEN: &str = "SYS_INCLUDE_PATH_SUFFIX_TOKEN";
const HEADER_EXTENSION_TOKEN: &str = "HEADER_EXTENSION_TOKEN";

fn quote_comment(text: &str) -> TokenStream {
    quote! { #NORMAL_COMMENT_PREFIX_TOKEN #text #NORMAL_COMMENT_SUFFIX_TOKEN }
}

fn quote_doc_comment(text: &str) -> TokenStream {
    quote! { #DOC_COMMENT_PREFIX_TOKEN #text #DOC_COMMENT_SUFFIX_TOKEN }
}

fn quote_hide_from_docs() -> TokenStream {
    let comment = quote_doc_comment("\\private");
    quote! {
        #NEWLINE_TOKEN
        #comment
    }
}

fn string_from_token_stream(token_stream: &TokenStream, source_path: Option<&Utf8Path>) -> String {
    let mut code = String::new();
    code.push_str(&format!("// {}\n", autogen_warning!()));
    if let Some(source_path) = source_path {
        code.push_str(&format!("// Based on {:?}.\n", format_path(source_path)));
    }

    code.push('\n');

    let generated_code = token_stream
        .to_string()
        .replace(&format!("{NEWLINE_TOKEN:?}"), "\n")
        .replace(NEWLINE_TOKEN, "\n") // Should only happen inside header extensions.
        .replace(&format!("{NORMAL_COMMENT_PREFIX_TOKEN:?} \""), "// ")
        .replace(&format!("\" {NORMAL_COMMENT_SUFFIX_TOKEN:?}"), "\n")
        .replace(&format!("{DOC_COMMENT_PREFIX_TOKEN:?} \""), "/// ")
        .replace(&format!("\" {DOC_COMMENT_SUFFIX_TOKEN:?}"), "\n")
        .replace(&format!("{ANGLE_BRACKET_LEFT_TOKEN:?} \""), "<")
        .replace(&format!("\" {ANGLE_BRACKET_RIGHT_TOKEN:?}"), ">")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace(" ::", "::");

    // Need to fix escaped quotes inside of comments.
    // Walk through all comments, replace and push to `code` as we go.
    let mut last_comment_end = 0;
    while let Some(comment_start) = generated_code[last_comment_end..].find("//") {
        code.push_str(&generated_code[last_comment_end..last_comment_end + comment_start]);
        let comment_start = last_comment_end + comment_start;
        let comment_end = comment_start + generated_code[comment_start..].find('\n').unwrap();
        let comment = &generated_code[comment_start..comment_end];
        let comment = comment.replace("\\\"", "\"");
        let comment = comment.replace("\\\\", "\\");
        code.push_str(&comment);
        last_comment_end = comment_end;
    }
    code.push_str(&generated_code[last_comment_end..]);

    code.push('\n');

    code
}

pub struct CppCodeGenerator {
    output_path: Utf8PathBuf,
}

impl crate::CodeGenerator for CppCodeGenerator {
    fn generate(
        &mut self,
        reporter: &Reporter,
        objects: &Objects,
        _arrow_registry: &ArrowRegistry,
    ) -> GeneratedFiles {
        re_tracing::profile_wait!("generate_folder");

        let scopes = objects
            .objects
            .values()
            .map(|obj| obj.scope())
            .collect::<HashSet<_>>();

        ObjectKind::ALL
            .par_iter()
            .filter(|&&object_kind| {
                // TODO(#5521): Implement view codegen for Rust.
                object_kind != ObjectKind::View
            })
            .flat_map(|object_kind| {
                scopes
                    .par_iter()
                    .flat_map(|scope| self.generate_folder(reporter, objects, scope, *object_kind))
            })
            .collect()
    }
}

impl CppCodeGenerator {
    pub fn new(output_path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            output_path: output_path.into(),
        }
    }

    fn generate_folder(
        &self,
        reporter: &Reporter,
        objects: &Objects,
        scope: &Option<String>,
        object_kind: ObjectKind,
    ) -> GeneratedFiles {
        let folder_name = if let Some(scope) = scope {
            format!("{}/{}", scope, object_kind.plural_snake_case())
        } else {
            object_kind.plural_snake_case().to_owned()
        };
        let folder_path_sdk = self.output_path.join("src/rerun").join(&folder_name);
        let folder_path_testing = self.output_path.join("tests/generated").join(&folder_name);

        let mut files_to_write = GeneratedFiles::default();

        // Generate folder contents:
        let objects_of_kind = objects
            .objects_of_kind(object_kind)
            .filter(|obj| &obj.scope() == scope)
            .collect_vec();

        for &obj in &objects_of_kind {
            if let Err(err) = generate_object_files(
                reporter,
                objects,
                &folder_path_sdk,
                &folder_path_testing,
                &mut files_to_write,
                obj,
            ) {
                reporter.error(&obj.virtpath, &obj.fqname, err);
            }
        }

        // Generate module file that includes all the headers:
        for testing in [false, true] {
            let hash = quote! { # };
            let pragma_once = pragma_once();
            let header_file_names = objects_of_kind
                .iter()
                .filter(|obj| obj.is_testing() == testing)
                .map(|obj| format!("{folder_name}/{}.hpp", obj.snake_case_name()))
                .collect_vec();
            if header_file_names.is_empty() {
                continue;
            }
            let tokens = quote! {
                #pragma_once
                #(#hash include #header_file_names "NEWLINE_TOKEN")*
            };
            let folder_path = if testing {
                &folder_path_testing
            } else {
                &folder_path_sdk
            };
            let filepath = folder_path.parent().unwrap().join(format!(
                "{}.hpp",
                object_kind.plural_snake_case().to_owned()
            ));
            let contents = string_from_token_stream(&tokens, None);
            files_to_write.insert(filepath, contents);
        }

        files_to_write
    }
}

fn generate_object_files(
    reporter: &Reporter,
    objects: &Objects,
    folder_path_sdk: &Utf8PathBuf,
    folder_path_testing: &Utf8PathBuf,
    files_to_write: &mut std::collections::BTreeMap<Utf8PathBuf, String>,
    obj: &Object,
) -> Result {
    let filename_stem = obj.snake_case_name();

    let mut hpp_includes = Includes::new(obj.fqname.clone(), obj.scope());
    hpp_includes.insert_system("cstdint");
    // we use `uint32_t` etc everywhere.
    hpp_includes.insert_rerun("result.hpp");
    // rerun result is used for serialization methods

    let (hpp_type_extensions, hpp_extension_string) =
        hpp_type_extensions(folder_path_sdk, &filename_stem, &mut hpp_includes);

    let (hpp, cpp) = generate_hpp_cpp(reporter, objects, obj, hpp_includes, &hpp_type_extensions)?;

    for (extension, tokens) in [("hpp", Some(hpp)), ("cpp", cpp)] {
        let Some(tokens) = tokens else {
            continue;
        };

        let mut contents = string_from_token_stream(&tokens, obj.relative_filepath());
        if let Some(hpp_extension_string) = &hpp_extension_string {
            contents = contents.replace(
                &format!("\"{HEADER_EXTENSION_TOKEN}\""), // NOLINT
                hpp_extension_string,
            );
        }
        let folder_path = if obj.is_testing() {
            folder_path_testing
        } else {
            folder_path_sdk
        };
        let filepath = folder_path.join(format!("{filename_stem}.{extension}"));
        let previous = files_to_write.insert(filepath, contents);
        assert!(
            previous.is_none(),
            "Multiple objects with the same name: {:?}",
            obj.name
        );
    }

    Ok(())
}

/// Retrieves code from an extension cpp file that should go to the generated header.
///
/// Additionally, picks up all includes files that aren't including the header itself.
///
/// Returns what to inject, and what to replace `HEADER_EXTENSION_TOKEN` with at the end.
fn hpp_type_extensions(
    folder_path: &Utf8Path,
    filename_stem: &str,
    includes: &mut Includes,
) -> (TokenStream, Option<String>) {
    let extension_file = folder_path.join(format!("{filename_stem}_ext.cpp"));
    let Ok(content) = std::fs::read_to_string(extension_file.as_std_path()) else {
        return (quote! {}, None);
    };

    const COPY_TO_HEADER_START_MARKER: &str = "<CODEGEN_COPY_TO_HEADER>";
    const COPY_TO_HEADER_END_MARKER: &str = "</CODEGEN_COPY_TO_HEADER>";

    let mut remaining_content = &content[..];
    let mut hpp_extension_string = String::new();

    while let Some(start) = remaining_content.find(COPY_TO_HEADER_START_MARKER) {
        let end = remaining_content.find(COPY_TO_HEADER_END_MARKER).unwrap_or_else(||
            panic!("C++ extension file has a start marker but no end marker. Expected to find '{COPY_TO_HEADER_END_MARKER}' in {extension_file:?}")
        );
        let end = remaining_content[..end].rfind('\n').unwrap_or_else(||
            panic!("Expected line break at some point before {COPY_TO_HEADER_END_MARKER} in {extension_file:?}")
        );

        let extensions = &remaining_content[start + COPY_TO_HEADER_START_MARKER.len()..end];

        // Comb through any includes in the extension string.
        for line in extensions.lines() {
            if line.starts_with("#include") {
                if let Some(start) = line.find('\"') {
                    let end = line.rfind('\"').unwrap_or_else(|| {
                        panic!(
                            "Expected to find ending '\"' in include line {line} in file {extension_file:?}"
                        )
                    });

                    includes.insert_relative(&line[start + 1..end]);
                } else if let Some(start) = line.find('<') {
                    let end = line.rfind('>').unwrap_or_else(|| {
                        panic!(
                        "Expected to find or '>' in include line {line} in file {extension_file:?}"
                    )
                    });
                    includes.insert_system(&line[start + 1..end]);
                } else {
                    panic!("Expected to find '\"' or '<' in include line {line} in file {extension_file:?}");
                }
            } else {
                hpp_extension_string += line;
                hpp_extension_string += "\n";
            }
        }

        remaining_content = &remaining_content[end + COPY_TO_HEADER_END_MARKER.len()..];
    }

    let start_comment = quote_comment(&format!(
        "START of extensions from {}:",
        extension_file.file_name().unwrap()
    ));
    let end_comment = quote_comment(&format!(
        "END of extensions from {}, start of generated code:",
        extension_file.file_name().unwrap()
    ));
    let hpp_type_extensions = quote! {
        public:  #start_comment
        #NEWLINE_TOKEN
        #HEADER_EXTENSION_TOKEN
        #NEWLINE_TOKEN
        #end_comment
        #NEWLINE_TOKEN
    };

    (hpp_type_extensions, Some(hpp_extension_string))
}

fn generate_hpp_cpp(
    reporter: &Reporter,
    objects: &Objects,
    obj: &Object,
    hpp_includes: Includes,
    hpp_type_extensions: &TokenStream,
) -> Result<(TokenStream, Option<TokenStream>)> {
    let QuotedObject { hpp, cpp } =
        QuotedObject::new(reporter, objects, obj, hpp_includes, hpp_type_extensions)?;
    let snake_case_name = obj.snake_case_name();
    let hash = quote! { # };
    let pragma_once = pragma_once();
    let header_file_name = format!("{snake_case_name}.hpp");

    let hpp = quote! {
        #pragma_once
        #hpp
    };
    let cpp = cpp.map(|cpp| {
        quote! {
            #hash include #header_file_name #NEWLINE_TOKEN #NEWLINE_TOKEN
            #cpp
        }
    });

    Ok((hpp, cpp))
}

fn pragma_once() -> TokenStream {
    let hash = quote! { # };
    quote! {
        #hash pragma once #NEWLINE_TOKEN #NEWLINE_TOKEN
    }
}

struct QuotedObject {
    hpp: TokenStream,
    cpp: Option<TokenStream>,
}

impl QuotedObject {
    #[allow(clippy::unnecessary_wraps)] // TODO(emilk): implement proper error handling instead of panicking
    pub fn new(
        reporter: &Reporter,
        objects: &Objects,
        obj: &Object,
        hpp_includes: Includes,
        hpp_type_extensions: &TokenStream,
    ) -> Result<Self> {
        match obj.class {
            ObjectClass::Struct => match obj.kind {
                ObjectKind::Datatype | ObjectKind::Component => Ok(Self::from_struct(
                    reporter,
                    objects,
                    obj,
                    hpp_includes,
                    hpp_type_extensions,
                )),
                ObjectKind::Archetype => Ok(Self::from_archetype(
                    reporter,
                    objects,
                    obj,
                    hpp_includes,
                    hpp_type_extensions,
                )),
                ObjectKind::View => {
                    // TODO(#5521): Implement view codegen for C++.
                    unimplemented!();
                }
            },
            ObjectClass::Enum => {
                if !hpp_type_extensions.is_empty() {
                    reporter.error(&obj.virtpath, &obj.fqname, "C++ enums cannot have type extensions, because C++ enums doesn't support member functions");
                }
                Ok(Self::from_enum(reporter, objects, obj, hpp_includes))
            }
            ObjectClass::Union => Ok(Self::from_union(
                reporter,
                objects,
                obj,
                hpp_includes,
                hpp_type_extensions,
            )),
        }
    }

    fn from_archetype(
        reporter: &Reporter,
        objects: &Objects,
        obj: &Object,
        mut hpp_includes: Includes,
        hpp_type_extensions: &TokenStream,
    ) -> Self {
        let type_ident = obj.ident();
        let quoted_docs = quote_obj_docs(reporter, objects, obj);

        let mut cpp_includes = Includes::new(obj.fqname.clone(), obj.scope());
        cpp_includes.insert_rerun("collection_adapter_builtins.hpp");
        hpp_includes.insert_system("utility"); // std::move
        hpp_includes.insert_rerun("indicator_component.hpp");

        let field_declarations = obj
            .fields
            .iter()
            .map(|obj_field| {
                let docstring = quote_field_docs(reporter, objects, obj_field);
                let field_name = field_name_identifier(obj_field);
                let field_type = quote_archetype_field_type(&mut hpp_includes, obj_field);
                let field_type = if obj_field.is_nullable {
                    hpp_includes.insert_system("optional");
                    quote! { std::optional<#field_type> }
                } else {
                    field_type
                };

                quote! {
                    #NEWLINE_TOKEN
                    #docstring
                    #field_type #field_name
                }
            })
            .collect_vec();

        let mut methods = Vec::new();

        let required_component_fields = obj
            .fields
            .iter()
            .filter(|field| !field.is_nullable)
            .collect_vec();

        // Constructors with all required components.
        if !required_component_fields.is_empty() && !obj.is_attr_set(ATTR_CPP_NO_FIELD_CTORS) {
            let (parameters, assignments): (Vec<_>, Vec<_>) = required_component_fields
                .iter()
                .map(|obj_field| {
                    let field_type = quote_archetype_field_type(&mut hpp_includes, obj_field);
                    let field_ident = field_name_identifier(obj_field);
                    // C++ compilers give warnings for re-using the same name as the member variable.
                    let parameter_ident = format_ident!("_{}", obj_field.name);
                    (
                        quote! { #field_type #parameter_ident },
                        quote! { #field_ident(std::move(#parameter_ident)) },
                    )
                })
                .unzip();

            methods.push(Method {
                // Making the constructor explicit prevents all sort of strange errors.
                // (e.g. `Points3D({{0.0f, 0.0f, 0.0f}})` would previously be ambiguous with the move constructor?!)
                declaration: MethodDeclaration::constructor(quote! {
                    explicit #type_ident(#(#parameters),*) : #(#assignments),*
                }),
                ..Method::default()
            });
        }

        // Builder methods for all optional components.
        for obj_field in obj.fields.iter().filter(|field| field.is_nullable) {
            let field_ident = field_name_identifier(obj_field);
            // C++ compilers give warnings for re-using the same name as the member variable.
            let parameter_ident = format_ident!("_{}", obj_field.name);
            let method_ident = format_ident!("with_{}", obj_field.name);
            let field_type = quote_archetype_field_type(&mut hpp_includes, obj_field);

            hpp_includes.insert_rerun("compiler_utils.hpp");
            let gcc_ignore_comment =
                quote_comment("See: https://github.com/rerun-io/rerun/issues/4027");

            methods.push(Method {
                docs: obj_field.docs.clone().into(),
                declaration: MethodDeclaration {
                    is_static: false,
                    return_type: quote!(#type_ident),
                    name_and_parameters: quote! {
                        #method_ident(#field_type #parameter_ident) &&
                    },
                },
                definition_body: quote! {
                    #field_ident = std::move(#parameter_ident);
                    #NEWLINE_TOKEN
                    #gcc_ignore_comment
                    RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
                },
                inline: true,
            });
        }

        let quoted_namespace = if let Some(scope) = obj.scope() {
            let scope = format_ident!("{}", scope);
            quote! { #scope::archetypes }
        } else {
            quote! {archetypes}
        };

        let serialize_method = archetype_serialize(&type_ident, obj, &mut hpp_includes);
        let serialize_hpp = serialize_method.to_hpp_tokens(reporter, objects);
        let serialize_cpp =
            serialize_method.to_cpp_tokens(&quote!(AsComponents<#quoted_namespace::#type_ident>));

        let methods_hpp = methods.iter().map(|m| m.to_hpp_tokens(reporter, objects));
        let methods_cpp = methods
            .iter()
            .map(|m| m.to_cpp_tokens(&quote!(#type_ident)));

        let indicator_comment = quote_doc_comment("Indicator component, used to identify the archetype when converting to a list of components.");
        let indicator_component_fqname =
            format!("{}Indicator", obj.fqname).replace("archetypes", "components");
        let doc_hide_comment = quote_hide_from_docs();
        let deprecation_notice = quote_deprecation_notice(obj);

        // Note that GCC doesn't like using deprecated fields even if the archetype itself is deprecated.
        // In that case we're just being generous with the ignore warnings, making it finegrained is hard and not really worth it anyways.
        let has_any_deprecated_fields = obj.fields.iter().any(|field| {
            field
                .typ
                .fqname()
                .and_then(|fqname| objects.get(fqname))
                .map_or(false, |obj| obj.deprecation_notice().is_some())
        });
        let (deprecation_ignore_start, deprecation_ignore_end) =
            quote_deprecation_ignore_start_and_end(
                &mut hpp_includes,
                obj.deprecation_notice().is_some() || has_any_deprecated_fields,
            );

        let hpp = quote! {
            #hpp_includes

            #deprecation_ignore_start

            namespace rerun::#quoted_namespace {
                #quoted_docs
                struct #deprecation_notice #type_ident {
                    #(#field_declarations;)*

                public:
                    static constexpr const char IndicatorComponentName[] = #indicator_component_fqname;
                    #NEWLINE_TOKEN
                    #NEWLINE_TOKEN
                    #indicator_comment
                    using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

                    #hpp_type_extensions

                public:
                    #type_ident() = default;
                    #type_ident(#type_ident&& other) = default;
                    #NEWLINE_TOKEN
                    #NEWLINE_TOKEN
                    #(#methods_hpp)*
                };
                #NEWLINE_TOKEN
                #NEWLINE_TOKEN
            }

            namespace rerun {
                // Instead of including as_components.hpp, simply re-declare the template since it's trivial
                #doc_hide_comment
                template<typename T>
                struct AsComponents;

                #deprecation_ignore_start

                #doc_hide_comment
                template<>
                struct AsComponents<#quoted_namespace::#type_ident> {
                    #serialize_hpp
                };
            }

            #deprecation_ignore_end
        };

        let cpp = quote! {
            #cpp_includes

            #deprecation_ignore_start

            namespace rerun::#quoted_namespace {
                #(#methods_cpp)*
            }

            namespace rerun {
                #NEWLINE_TOKEN
                #NEWLINE_TOKEN
                #serialize_cpp
            }

            #deprecation_ignore_end
        };

        Self {
            hpp,
            cpp: Some(cpp),
        }
    }

    fn from_struct(
        reporter: &Reporter,
        objects: &Objects,
        obj: &Object,
        mut hpp_includes: Includes,
        hpp_type_extensions: &TokenStream,
    ) -> Self {
        let namespace_ident = obj.namespace_ident();

        let quoted_namespace = if let Some(scope) = obj.scope() {
            let scope = format_ident!("{}", scope);
            quote! { #scope::#namespace_ident}
        } else {
            quote! {#namespace_ident}
        };

        let type_ident = obj.ident();
        let quoted_docs = quote_obj_docs(reporter, objects, obj);
        let deprecation_notice = quote_deprecation_notice(obj);

        let mut cpp_includes = Includes::new(obj.fqname.clone(), obj.scope());
        let mut hpp_declarations = ForwardDecls::default();

        let field_declarations = obj
            .fields
            .iter()
            .map(|obj_field| {
                let declaration = quote_variable_with_docstring(
                    reporter,
                    objects,
                    &mut hpp_includes,
                    obj_field,
                    &field_name_identifier(obj_field),
                );
                quote! {
                    #NEWLINE_TOKEN
                    #declaration
                }
            })
            .collect_vec();

        let mut methods = Vec::new();

        if obj.fields.len() == 1 && !obj.is_attr_set(ATTR_CPP_NO_FIELD_CTORS) {
            methods.extend(single_field_constructor_methods(
                obj,
                &mut hpp_includes,
                objects,
            ));
        };

        // If we're a component with a single datatype field, add an implicit casting operator for convenience.
        if obj.kind == ObjectKind::Component
            && obj.fields.len() == 1
            && matches!(obj.fields[0].typ, Type::Object { .. })
        {
            if let Type::Object(datatype_fqname) = &obj.fields[0].typ {
                let data_type = quote_field_type(&mut hpp_includes, &obj.fields[0]);
                let type_name = datatype_fqname.split('.').last().unwrap();
                let field_name = format_ident!("{}", obj.fields[0].name);

                methods.push(Method {
                    docs: format!("Cast to the underlying {type_name} datatype").into(),
                    declaration: MethodDeclaration {
                        name_and_parameters: quote! { operator #data_type() const },
                        is_static: false,
                        return_type: quote! {},
                    },
                    definition_body: quote! {
                        return #field_name;
                    },
                    inline: true,
                });
            }
        }

        let methods_hpp = methods.iter().map(|m| m.to_hpp_tokens(reporter, objects));

        let (hpp_loggable, cpp_loggable) = quote_loggable_hpp_and_cpp(
            reporter,
            obj,
            objects,
            &mut hpp_includes,
            &mut cpp_includes,
            &mut hpp_declarations,
        );

        let hpp = quote! {
            #hpp_includes

            #hpp_declarations

            namespace rerun::#quoted_namespace {
                #quoted_docs
                struct #deprecation_notice #type_ident {
                    #(#field_declarations;)*

                    #hpp_type_extensions

                public:
                    #type_ident() = default;
                    #NEWLINE_TOKEN
                    #NEWLINE_TOKEN
                    #(#methods_hpp)*
                };
            }

            #hpp_loggable
        };

        let cpp = if cpp_loggable.is_some() || methods.iter().any(|m| !m.inline) {
            let methods_cpp = methods
                .iter()
                .map(|m| m.to_cpp_tokens(&quote!(#type_ident)));

            Some(quote! {
                #cpp_includes

                namespace rerun::#quoted_namespace {
                    #(#methods_cpp)*
                }

                #cpp_loggable
            })
        } else {
            None
        };

        Self { hpp, cpp }
    }

    fn from_union(
        reporter: &Reporter,
        objects: &Objects,
        obj: &Object,
        mut hpp_includes: Includes,
        hpp_type_extensions: &TokenStream,
    ) -> Self {
        // We implement sum-types as tagged unions;
        // Putting non-POD types in a union requires C++11.
        //
        // enum class Rotation3DTag : uint8_t {
        //     None = 0,
        //     Quaternion,
        //     AxisAngle,
        // };
        //
        // union Rotation3DData {
        //     Quaternion quaternion;
        //     AxisAngle axis_angle;
        // };
        //
        // struct Rotation3D {
        //     Rotation3DTag _tag;
        //     Rotation3DData _data;
        // };

        assert!(
            obj.kind != ObjectKind::Archetype,
            "Union archetypes are not supported {}",
            obj.fqname
        );
        let namespace_ident = obj.namespace_ident();
        let quoted_namespace = if let Some(scope) = obj.scope() {
            let scope = format_ident!("{}", scope);
            quote! { #scope::#namespace_ident}
        } else {
            quote! {#namespace_ident}
        };

        let pascal_case_name = &obj.name;
        let pascal_case_ident = obj.ident();
        let quoted_docs = quote_obj_docs(reporter, objects, obj);
        let deprecation_notice = quote_deprecation_notice(obj);

        let tag_typename = format_ident!("{pascal_case_name}Tag");
        let data_typename = format_ident!("{pascal_case_name}Data");

        let tag_fields = std::iter::once({
            let comment = quote_doc_comment(
                "Having a special empty state makes it possible to implement move-semantics. \
                We need to be able to leave the object in a state which we can run the destructor on.");
            let tag_name = format_ident!("None");
            quote! {
                #NEWLINE_TOKEN
                #comment
                #tag_name = 0,
            }
        })
        .chain(obj.fields.iter().map(|obj_field| {
            let ident = field_name_identifier(obj_field);
            quote! {
                #ident,
            }
        }))
        .collect_vec();

        hpp_includes.insert_system("utility"); // std::move
        hpp_includes.insert_system("cstring"); // std::memcpy

        let mut cpp_includes = Includes::new(obj.fqname.clone(), obj.scope());
        #[allow(unused)]
        let mut hpp_declarations = ForwardDecls::default();

        let enum_data_declarations = obj
            .fields
            .iter()
            .filter(|obj_field| obj_field.typ != Type::Unit)
            .map(|obj_field| {
                let declaration = quote_variable_with_docstring(
                    reporter,
                    objects,
                    &mut hpp_includes,
                    obj_field,
                    &format_ident!("{}", obj_field.snake_case_name()),
                );
                quote! {
                    #NEWLINE_TOKEN
                    #declaration
                }
            })
            .collect_vec();

        let mut methods = Vec::new();

        if !obj.is_attr_set(ATTR_CPP_NO_FIELD_CTORS) {
            if are_types_disjoint(&obj.fields) {
                // Implicit construct from the different variant types:
                for obj_field in &obj.fields {
                    if obj_field.typ == Type::Unit {
                        // Can't create a constructor for a unit type.
                        continue;
                    }

                    let snake_case_ident = format_ident!("{}", obj_field.snake_case_name());
                    let param_declaration =
                        quote_variable(&mut hpp_includes, obj_field, &snake_case_ident);
                    let definition_body = quote!(*this = #pascal_case_ident::#snake_case_ident(std::move(#snake_case_ident)););

                    methods.push(Method {
                        docs: obj_field.docs.clone().into(),
                        declaration: MethodDeclaration::constructor(
                            quote!(#pascal_case_ident(#param_declaration) : #pascal_case_ident()),
                        ),
                        definition_body,
                        inline: true,
                    });
                }
            } else {
                // Cannot make implicit constructors, e.g. for
                // `enum Angle { Radians(f32), Degrees(f32) };`
            }
        }

        // Add one static constructor for every field.
        for obj_field in &obj.fields {
            methods.push(static_constructor_for_enum_type(
                &mut hpp_includes,
                obj_field,
                &pascal_case_ident,
                &tag_typename,
            ));
        }

        // Code that allows to access the data of the union in a safe way.
        for obj_field in &obj.fields {
            let snake_case_name = obj_field.snake_case_name();
            let field_name = format_ident!("{}", snake_case_name);
            let tag_name = field_name_identifier(obj_field);

            let method = if obj_field.typ == Type::Unit {
                let method_name = format_ident!("is_{}", snake_case_name);
                Method {
                    docs: format!("Returns true if the union is in the {snake_case_name} state.")
                        .into(),
                    declaration: MethodDeclaration {
                        name_and_parameters: quote! { #method_name() const },
                        return_type: quote! { bool },
                        is_static: false,
                    },
                    definition_body: quote! {
                        return _tag == detail::#tag_typename::#tag_name;
                    },
                    inline: true,
                }
            } else {
                let typ = quote_field_type(&mut hpp_includes, obj_field);
                let method_name = format_ident!("get_{}", snake_case_name);
                Method {
                    docs: format!("Return a pointer to {snake_case_name} if the union is in that state, otherwise `nullptr`.").into(),
                    declaration: MethodDeclaration {
                        name_and_parameters: quote! { #method_name() const },
                        return_type: quote! { const #typ* },
                        is_static: false,
                    },
                    definition_body: quote! {
                        if (_tag == detail::#tag_typename::#tag_name) {
                            return &_data.#field_name;
                        } else {
                            return nullptr;
                        }
                    },
                    inline: true,
                }
            };
            methods.push(method);
        }

        let destructor = if obj.has_default_destructor(objects) {
            // No destructor needed
            quote! {}
        } else {
            let destructor_match_arms = std::iter::once({
                let comment = quote_comment("Nothing to destroy");
                quote! {
                    case detail::#tag_typename::None: {
                        #NEWLINE_TOKEN
                        #comment
                    } break;
                }
            })
            .chain(obj.fields.iter().map(|obj_field| {
                let tag_ident = field_name_identifier(obj_field);
                let field_ident = format_ident!("{}", obj_field.snake_case_name());

                if obj_field.typ.has_default_destructor(objects) {
                    let comment = quote_comment("has a trivial destructor");
                    quote! {
                        case detail::#tag_typename::#tag_ident: {
                            #NEWLINE_TOKEN
                            #comment
                        } break;
                    }
                } else {
                    let typ = quote_field_type(&mut hpp_includes, obj_field);
                    hpp_includes.insert_system("utility"); // std::move
                    quote! {
                        case detail::#tag_typename::#tag_ident: {
                            using TypeAlias = #typ;
                            _data.#field_ident.~TypeAlias();
                        } break;
                    }
                }
            }))
            .collect_vec();

            quote! {
                ~#pascal_case_ident() {
                    switch (this->_tag) {
                        #(#destructor_match_arms)*

                        default:
                            assert(false && "unreachable");
                    }
                }
            }
        };

        let copy_constructor = {
            // Note that `switch` on an enum without handling all cases causes `-Wswitch-enum` warning!
            let mut placement_new_arms = Vec::new();
            let mut trivial_memcpy_cases = Vec::new();
            for obj_field in &obj.fields {
                let tag_ident = field_name_identifier(obj_field);
                let case = quote!(case detail::#tag_typename::#tag_ident:);

                // Inferring from trivial destructability that we don't need to call the copy constructor is a little bit wonky,
                // but is typically the reason why we need to do this in the first place - if we'd always memcpy we'd get double-free errors.
                // (As with swap, we generously assume that objects are rellocatable)
                if obj_field.typ.has_default_destructor(objects) {
                    trivial_memcpy_cases.push(case);
                } else {
                    // the `this->_data` union is not yet initialized, so we must use placement new:
                    let typ = quote_field_type(&mut hpp_includes, obj_field);
                    hpp_includes.insert_system("new"); // placement-new

                    let field_ident = format_ident!("{}", obj_field.snake_case_name());
                    placement_new_arms.push(quote! {
                        #case {
                            using TypeAlias = #typ;
                            new (&_data.#field_ident) TypeAlias(other._data.#field_ident);
                        } break;
                    });
                }
            }

            let trivial_memcpy = quote! {
                const void* otherbytes = reinterpret_cast<const void*>(&other._data);
                void* thisbytes = reinterpret_cast<void*>(&this->_data);
                std::memcpy(thisbytes, otherbytes, sizeof(detail::#data_typename));
            };

            let comment = quote_doc_comment("Copy constructor");

            if placement_new_arms.is_empty() {
                quote! {
                    #NEWLINE_TOKEN
                    #NEWLINE_TOKEN
                    #comment
                    #pascal_case_ident(const #pascal_case_ident& other) : _tag(other._tag) {
                        #trivial_memcpy
                    }
                }
            } else if trivial_memcpy_cases.is_empty() {
                quote! {
                    #NEWLINE_TOKEN
                    #NEWLINE_TOKEN
                    #comment
                    #pascal_case_ident(const #pascal_case_ident& other) : _tag(other._tag) {
                        switch (other._tag) {
                            #(#placement_new_arms)*

                            case detail::#tag_typename::None: {
                                // there is nothing to copy
                            } break;

                            default:
                                assert(false && "unreachable");
                        }
                    }
                }
            } else {
                quote! {
                    #NEWLINE_TOKEN
                    #NEWLINE_TOKEN
                    #comment
                    #pascal_case_ident(const #pascal_case_ident& other) : _tag(other._tag) {
                        switch (other._tag) {
                            #(#placement_new_arms)*

                            #(#trivial_memcpy_cases)* {
                                #trivial_memcpy
                            } break;

                            case detail::#tag_typename::None: {
                                // there is nothing to copy
                            } break;

                            default:
                                assert(false && "unreachable");
                        }
                    }
                }
            }
        };

        let swap_comment = quote_comment("This bitwise swap would fail for self-referential types, but we don't have any of those.");
        let hide_from_docs_comment = quote_hide_from_docs();

        let (hpp_loggable, cpp_loggable) = quote_loggable_hpp_and_cpp(
            reporter,
            obj,
            objects,
            &mut hpp_includes,
            &mut cpp_includes,
            &mut hpp_declarations,
        );

        let methods_hpp = methods.iter().map(|m| m.to_hpp_tokens(reporter, objects));
        let hpp = quote! {
            #hpp_includes

            #hpp_declarations

            namespace rerun::#quoted_namespace {
                namespace detail {
                    #hide_from_docs_comment
                    enum class #tag_typename : uint8_t {
                        #(#tag_fields)*
                    };

                    #hide_from_docs_comment
                    union #data_typename {
                        #(#enum_data_declarations;)*

                        // Required by static constructors
                        #data_typename() {
                            std::memset(reinterpret_cast<void*>(this), 0, sizeof(#data_typename));
                        }
                        ~#data_typename() { }

                        // Note that this type is *not* copyable unless all enum fields are trivially destructable.

                        void swap(#data_typename& other) noexcept {
                            #NEWLINE_TOKEN
                            #swap_comment
                            char temp[sizeof(#data_typename)];
                            void* otherbytes = reinterpret_cast<void*>(&other);
                            void* thisbytes = reinterpret_cast<void*>(this);
                            std::memcpy(temp, thisbytes, sizeof(#data_typename));
                            std::memcpy(thisbytes, otherbytes, sizeof(#data_typename));
                            std::memcpy(otherbytes, temp, sizeof(#data_typename));
                        }
                    };
                }

                #quoted_docs
                struct #deprecation_notice #pascal_case_ident {
                    #pascal_case_ident() : _tag(detail::#tag_typename::None) {}

                    #copy_constructor

                    // Copy-assignment
                    #pascal_case_ident& operator=(const #pascal_case_ident& other) noexcept {
                        #pascal_case_ident tmp(other);
                        this->swap(tmp);
                        return *this;
                    }

                    // Move-constructor:
                    #pascal_case_ident(#pascal_case_ident&& other) noexcept : #pascal_case_ident() {
                        this->swap(other);
                    }

                    // Move-assignment:
                    #pascal_case_ident& operator=(#pascal_case_ident&& other) noexcept {
                        this->swap(other);
                        return *this;
                    }

                    #destructor

                    #hpp_type_extensions

                    // This is useful for easily implementing the move constructor and assignment operators:
                    void swap(#pascal_case_ident& other) noexcept {
                        // Swap tags: Not using std::swap here causes a warning for some gcc version about potentially uninitialized data.
                        std::swap(this->_tag, other._tag);

                        // Swap data:
                        this->_data.swap(other._data);
                    }

                    #(#methods_hpp)*

                    #hide_from_docs_comment
                    const detail::#data_typename& get_union_data() const { return _data; }

                    #hide_from_docs_comment
                    detail::#tag_typename get_union_tag() const { return _tag; }

                private:
                    detail::#tag_typename _tag;
                    detail::#data_typename _data;
                };
            }

            #hpp_loggable
        };

        let cpp_methods = methods
            .iter()
            .map(|m| m.to_cpp_tokens(&quote!(#pascal_case_ident)));
        let cpp = quote! {
            #cpp_includes

            namespace rerun::#quoted_namespace {
                #(#cpp_methods)*
            }

            #cpp_loggable
        };

        Self {
            hpp,
            cpp: Some(cpp),
        }
    }

    // C-style enum
    fn from_enum(
        reporter: &Reporter,
        objects: &Objects,
        obj: &Object,
        mut hpp_includes: Includes,
    ) -> Self {
        // We use a simple `enum class`, which is a type-safe enum.
        // They don't support methods, but we don't need them,
        // since `Loggable` is implemented outside the type.

        let namespace_ident = obj.namespace_ident();

        let quoted_namespace = if let Some(scope) = obj.scope() {
            let scope = format_ident!("{}", scope);
            quote! { #scope::#namespace_ident}
        } else {
            quote! {#namespace_ident}
        };

        let type_ident = obj.ident();
        let quoted_docs = quote_obj_docs(reporter, objects, obj);
        let deprecation_notice = quote_deprecation_notice(obj);

        let mut cpp_includes = Includes::new(obj.fqname.clone(), obj.scope());
        let mut hpp_declarations = ForwardDecls::default();

        let field_declarations = obj
            .fields
            .iter()
            .map(|obj_field| {
                let enum_value = obj_field.enum_value.unwrap();
                let docstring = quote_field_docs(reporter, objects, obj_field);
                let field_name = field_name_identifier(obj_field);

                // We assign the arrow type index to the enum fields to make encoding simpler and faster:
                let arrow_type_index = Literal::usize_unsuffixed(enum_value as _);

                quote! {
                    #NEWLINE_TOKEN
                    #docstring
                    #field_name = #arrow_type_index
                }
            })
            .collect_vec();

        let (hpp_loggable, cpp_loggable) = quote_loggable_hpp_and_cpp(
            reporter,
            obj,
            objects,
            &mut hpp_includes,
            &mut cpp_includes,
            &mut hpp_declarations,
        );

        let hpp = quote! {
            #hpp_includes

            #hpp_declarations

            namespace rerun::#quoted_namespace {
                #quoted_docs
                enum class #deprecation_notice #type_ident : uint8_t {
                    #(#field_declarations,)*
                };
            }

            #hpp_loggable
        };
        let cpp = quote! {
            #cpp_includes

            #cpp_loggable
        };

        Self {
            hpp,
            cpp: Some(cpp),
        }
    }
}

fn field_name_identifier(obj_field: &ObjectField) -> Ident {
    if let Some(name) = obj_field.try_get_attr::<String>(ATTR_CPP_RENAME_FIELD) {
        format_ident!("{}", name)
    } else {
        format_ident!("{}", obj_field.name)
    }
}

fn single_field_constructor_methods(
    obj: &Object,
    hpp_includes: &mut Includes,
    objects: &Objects,
) -> Vec<Method> {
    let field = &obj.fields[0];
    let type_ident = obj.ident();

    let mut methods =
        add_copy_assignment_and_constructor(hpp_includes, field, field, &type_ident, objects);

    // If the field is a custom type as well which in turn has only a single field,
    // provide a constructor for that single field as well.
    //
    // Note that we previously we tried to do a general forwarding constructor via variadic templates,
    // but ran into some issues when init archetypes with initializer lists.
    if let Type::Object(field_type_fqname) = &field.typ {
        let field_type_obj = &objects[field_type_fqname];
        if field_type_obj.fields.len() == 1 && !field_type_obj.is_attr_set(ATTR_CPP_NO_FIELD_CTORS)
        {
            methods.extend(add_copy_assignment_and_constructor(
                hpp_includes,
                &field_type_obj.fields[0],
                field,
                &type_ident,
                objects,
            ));
        }
    }

    methods
}

fn add_copy_assignment_and_constructor(
    hpp_includes: &mut Includes,
    obj_field: &ObjectField,
    target_field: &ObjectField,
    type_ident: &Ident,
    objects: &Objects,
) -> Vec<Method> {
    let mut methods = Vec::new();
    let field_ident = field_name_identifier(target_field);
    let param_ident = format_ident!("{}_", obj_field.name);

    // We keep parameter passing for assignment & ctors simple by _always_ passing by value.
    // The basic assumption is that anything that has an expensive copy has a move constructor
    // and move constructors are cheap.
    //
    // Note that in this setup there's either
    // - 1 move, 1 copy: If an lvalue is passed gets copied into the value and then moved into the field.
    // - 2 move: If a temporary is passed it gets moved into the value and then moved again into the field.
    //
    // Also good to know:
    // In x64 and aarch64 (and others) structs are usually passed by pointer to stack _anyways_!
    // - everything above 8 bytes in x64:
    //   https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention?view=msvc-170#parameter-passing
    // - everything above 16 bytes (plus extra rules for float structs) in aarch64
    //   https://devblogs.microsoft.com/oldnewthing/20220823-00/?p=107041

    let typ = quote_field_type(hpp_includes, obj_field);

    let copy_or_move = if obj_field.typ.has_default_destructor(objects) {
        quote!(#param_ident)
    } else {
        hpp_includes.insert_system("utility"); // std::move
        quote!(std::move(#param_ident))
    };
    methods.push(Method {
        declaration: MethodDeclaration::constructor(quote! {
            #type_ident(#typ #param_ident) : #field_ident(#copy_or_move)
        }),
        ..Method::default()
    });
    methods.push(Method {
        declaration: MethodDeclaration {
            is_static: false,
            return_type: quote!(#type_ident&),
            name_and_parameters: quote! {
                operator=(#typ #param_ident)
            },
        },
        definition_body: quote! {
            #field_ident = #copy_or_move;
            return *this;
        },
        ..Method::default()
    });

    methods
}

/// If the type forwards to another rerun defined type, returns the fully qualified name of that type.
fn transparent_forwarded_fqname(obj: &Object) -> Option<&str> {
    if obj.is_arrow_transparent() && obj.fields.len() == 1 && !obj.fields[0].is_nullable {
        if let Type::Object(fqname) = &obj.fields[0].typ {
            return Some(fqname);
        }
    }
    None
}

fn arrow_data_type_method(
    obj: &Object,
    objects: &Objects,
    hpp_includes: &mut Includes,
    cpp_includes: &mut Includes,
    hpp_declarations: &mut ForwardDecls,
) -> Method {
    hpp_includes.insert_system("memory"); // std::shared_ptr

    let (inline, definition_body) =
        if let Some(forwarded_fqname) = transparent_forwarded_fqname(obj) {
            let forwarded_type = quote_fqname_as_type_path(hpp_includes, forwarded_fqname);
            (
                true,
                quote! {
                    return Loggable<#forwarded_type>::arrow_datatype();
                },
            )
        } else {
            cpp_includes.insert_system("arrow/type_fwd.h");
            hpp_declarations.insert("arrow", ForwardDecl::Class(format_ident!("DataType")));

            let quoted_datatype = quote_arrow_datatype(
                &Type::Object(obj.fqname.clone()),
                objects,
                cpp_includes,
                true,
            );

            (
                false,
                quote! {
                    static const auto datatype = #quoted_datatype;
                    return datatype;
                },
            )
        };

    Method {
        docs: "Returns the arrow data type this type corresponds to.".into(),
        declaration: MethodDeclaration {
            is_static: true,
            return_type: quote! { const std::shared_ptr<arrow::DataType>& },
            name_and_parameters: quote! { arrow_datatype() },
        },
        definition_body,
        inline,
    }
}

fn fill_arrow_array_builder_method(
    obj: &Object,
    cpp_includes: &mut Includes,
    hpp_declarations: &mut ForwardDecls,
    objects: &Objects,
) -> Method {
    cpp_includes.insert_system("arrow/builder.h");

    let builder = format_ident!("builder");
    let arrow_builder_type = arrow_array_builder_type_object(obj, objects, hpp_declarations);

    let fill_builder = quote_fill_arrow_array_builder(obj, objects, &builder, cpp_includes);

    let type_ident = obj.ident();
    let namespace_ident = obj.namespace_ident();
    let quoted_namespace = if let Some(scope) = obj.scope() {
        let scope = format_ident!("{}", scope);
        quote! { #scope::#namespace_ident}
    } else {
        quote! {#namespace_ident}
    };

    Method {
        docs: "Fills an arrow array builder with an array of this type.".into(),
        declaration: MethodDeclaration {
            is_static: true,
            return_type: quote! { rerun::Error },
            // TODO(andreas): Pass in validity map.
            name_and_parameters: quote! {
                fill_arrow_array_builder(arrow::#arrow_builder_type* #builder, const #quoted_namespace::#type_ident* elements, size_t num_elements)
            },
        },
        definition_body: quote! {
            #fill_builder
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
            return Error::ok();
        },
        inline: false,
    }
}

fn to_arrow_method(
    obj: &Object,
    objects: &Objects,
    hpp_includes: &mut Includes,
    declarations: &mut ForwardDecls,
) -> Method {
    hpp_includes.insert_system("memory"); // std::shared_ptr

    let type_ident = obj.ident();
    let namespace_ident = obj.namespace_ident();

    let quoted_namespace = if let Some(scope) = obj.scope() {
        let scope = format_ident!("{}", scope);
        quote! { #scope::#namespace_ident}
    } else {
        quote! {#namespace_ident}
    };

    let (inline, definition_body) = if let Some(forwarded_fqname) =
        transparent_forwarded_fqname(obj)
    {
        let forwarded_type = quote_fqname_as_type_path(hpp_includes, forwarded_fqname);
        let field_name = format_ident!("{}", obj.fields[0].snake_case_name());

        (
            true,
            quote! {
                if (num_instances == 0) {
                    return Loggable<#forwarded_type>::to_arrow(nullptr, 0);
                } else if (instances == nullptr) {
                    return rerun::Error(ErrorCode::UnexpectedNullArgument, "Passed array instances is null when num_elements > 0.");
                } else {
                    return Loggable<#forwarded_type>::to_arrow(&instances->#field_name, num_instances);
                }
            },
        )
    } else {
        hpp_includes.insert_rerun("result.hpp");
        declarations.insert("arrow", ForwardDecl::Class(format_ident!("Array")));

        let todo_pool = quote_comment("TODO(andreas): Allow configuring the memory pool.");

        // Only need this in the cpp file where we don't need to forward declare the arrow builder type.
        let arrow_builder_type =
            arrow_array_builder_type_object(obj, objects, &mut ForwardDecls::default());

        (
            false,
            quote! {
                #NEWLINE_TOKEN
                #todo_pool
                arrow::MemoryPool* pool = arrow::default_memory_pool();
                auto datatype = arrow_datatype();
                #NEWLINE_TOKEN
                #NEWLINE_TOKEN
                ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
                if (instances && num_instances > 0) {
                    RR_RETURN_NOT_OK(Loggable<#quoted_namespace::#type_ident>::fill_arrow_array_builder(
                        static_cast<arrow::#arrow_builder_type*>(builder.get()),
                        instances,
                        num_instances
                    ));
                }
                std::shared_ptr<arrow::Array> array;
                ARROW_RETURN_NOT_OK(builder->Finish(&array));
                return array;
            },
        )
    };

    Method {
        docs: format!(
            "Serializes an array of `rerun::{quoted_namespace}::{type_ident}` into an arrow array."
        )
        .into(),
        declaration: MethodDeclaration {
            is_static: true,
            return_type: quote! { Result<std::shared_ptr<arrow::Array>> },
            name_and_parameters: quote! {
                to_arrow(const #quoted_namespace::#type_ident* instances, size_t num_instances)
            },
        },
        definition_body,
        inline,
    }
}

fn archetype_serialize(type_ident: &Ident, obj: &Object, hpp_includes: &mut Includes) -> Method {
    hpp_includes.insert_rerun("component_batch.hpp");
    hpp_includes.insert_rerun("collection.hpp");
    hpp_includes.insert_system("vector"); // std::vector

    let quoted_scoped_archetypes = if let Some(scope) = obj.scope() {
        let scope = format_ident!("{scope}");
        quote!(#scope::archetypes)
    } else {
        quote!(archetypes)
    };

    let archetype_name = &obj.fqname;

    let num_fields = quote_integer(obj.fields.len() + 1); // Plus one for the indicator.
    let push_batches = obj.fields.iter().map(|field| {
        let field_name = field_name_identifier(field);
        let field_fqname = &field.typ.fqname();
        let field_accessor = quote!(archetype.#field_name);

        let push_back = quote! {
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        };

        let archetype_field_name = field.snake_case_name();

        // TODO(andreas): Introducing MonoCollection will remove the need for distinguishing these two cases.
        if field.is_nullable && !obj.attrs.has(ATTR_RERUN_LOG_MISSING_AS_EMPTY) {
            quote! {
                if (#field_accessor.has_value()) {
                    auto result = ComponentBatch::from_loggable(
                        #field_accessor.value(),
                        ComponentDescriptor(#archetype_name, #archetype_field_name, #field_fqname)
                    );
                    #push_back
                }
            }
        } else {
            quote! {
                {
                    auto result = ComponentBatch::from_loggable(
                        #field_accessor,
                        ComponentDescriptor(#archetype_name, #archetype_field_name, #field_fqname)
                    );
                    #push_back
                }
            }
        }
    });

    Method {
        docs: "Serialize all set component batches.".into(),
        declaration: MethodDeclaration {
            is_static: true,
            // TODO(andreas): Use a rerun::Collection here as well.
            return_type: quote!(Result<std::vector<ComponentBatch>>),
            name_and_parameters: quote!(serialize(const #quoted_scoped_archetypes::#type_ident& archetype)),
        },
        definition_body: quote! {
            using namespace #quoted_scoped_archetypes;
            #NEWLINE_TOKEN
            std::vector<ComponentBatch> cells;
            cells.reserve(#num_fields);
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
            #(#push_batches)*
            {
                auto indicator = #type_ident::IndicatorComponent();
                auto result = ComponentBatch::from_loggable(indicator);
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
            return cells;
        },
        inline: false,
    }
}

fn quote_fill_arrow_array_builder(
    obj: &Object,
    objects: &Objects,
    builder: &Ident,
    includes: &mut Includes,
) -> TokenStream {
    let type_ident = obj.ident();
    let namespace_ident = obj.namespace_ident();
    let quoted_namespace = if let Some(scope) = obj.scope() {
        let scope = format_ident!("{}", scope);
        quote! { #scope::#namespace_ident}
    } else {
        quote! {#namespace_ident}
    };

    let parameter_check = quote! {
        if (builder == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
        }
        if (elements == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Cannot serialize null pointer to arrow array.");
        }
        #NEWLINE_TOKEN
        #NEWLINE_TOKEN
    };

    if obj.is_arrow_transparent() {
        let field = &obj.fields[0];
        if let Type::Object(fqname) = &field.typ {
            if field.is_nullable {
                quote! {
                    (void)builder;
                    (void)elements;
                    (void)num_elements;
                    if (true) { // Works around unreachability compiler warning.
                        return rerun::Error(ErrorCode::NotImplemented, "TODO(andreas) Handle nullable extensions");
                    }
                }
            } else {
                // Trivial forwarding to inner type.
                let quoted_fqname = quote_fqname_as_type_path(includes, fqname);
                quote! {
                    static_assert(sizeof(#quoted_fqname) == sizeof(#quoted_namespace::#type_ident));
                    RR_RETURN_NOT_OK(Loggable<#quoted_fqname>::fill_arrow_array_builder(
                        builder, reinterpret_cast<const #quoted_fqname*>(elements), num_elements
                    ));
                }
            }
        } else {
            let append_to_builder =
                quote_append_field_to_builder(&obj.fields[0], builder, true, includes, objects);
            quote! {
                #parameter_check
                #append_to_builder
            }
        }
    } else {
        match obj.class {
            ObjectClass::Struct => {
                let fill_fields = obj.fields.iter().enumerate().map(
                    |(field_index, field)| {
                        let field_index = quote_integer(field_index);
                        let field_builder = format_ident!("field_builder");
                        let field_builder_type = arrow_array_builder_type(&field.typ, objects);
                        let field_append = quote_append_field_to_builder(field, &field_builder, false, includes, objects);
                        quote! {
                            {
                                auto #field_builder = static_cast<arrow::#field_builder_type*>(builder->field_builder(#field_index));
                                #field_append
                            }
                        }
                    },
                );

                quote! {
                    #parameter_check
                    #(#fill_fields)*
                    #NEWLINE_TOKEN
                    ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));
                }
            }

            // C-style enum, encoded as a sparse arrow union
            ObjectClass::Enum => {
                quote! {
                    #parameter_check
                    ARROW_RETURN_NOT_OK(#builder->Reserve(static_cast<int64_t>(num_elements)));
                    for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                        const auto variant = elements[elem_idx];
                        ARROW_RETURN_NOT_OK(#builder->Append(static_cast<uint8_t>(variant)));
                    }
                }
            }

            // sum-type union, encoded as a dense arrow union
            ObjectClass::Union => {
                let variant_builder = format_ident!("variant_builder");

                let tag_cases = obj.fields
                .iter()
                .map(|variant| {
                    let arrow_builder_type = arrow_array_builder_type(&variant.typ, objects);
                    let variant_name = format_ident!("{}", variant.name);

                    let variant_append = if let Some(element_type) = variant.typ.plural_inner() {
                        if variant.is_nullable {
                            let error = format!("Failed to serialize {}::{}: nullable list types in unions not yet implemented", obj.name, variant.name);
                            quote! {
                                (void)#variant_builder;
                                return rerun::Error(ErrorCode::NotImplemented, #error);
                            }
                        } else if arrow_builder_type == "ListBuilder" {
                            let field_name = format_ident!("{}", variant.snake_case_name());

                            if *element_type == ElementType::Float16 {
                                // We need an extra cast for float16:
                                quote! {
                                    ARROW_RETURN_NOT_OK(variant_builder->Append());
                                    auto value_builder =
                                        static_cast<arrow::HalfFloatBuilder *>(variant_builder->value_builder());
                                    const rerun::half* values = union_instance.get_union_data().#field_name.data();
                                    ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                                        reinterpret_cast<const uint16_t*>(values),
                                        static_cast<int64_t>(union_instance.get_union_data().#field_name.size())
                                    ));
                                }
                            } else {
                                let type_builder_name = match element_type {
                                    ElementType::UInt8 => Some("UInt8Builder"),
                                    ElementType::UInt16 => Some("UInt16Builder"),
                                    ElementType::UInt32 => Some("UInt32Builder"),
                                    ElementType::UInt64 => Some("UInt64Builder"),
                                    ElementType::Int8 => Some("Int8Builder"),
                                    ElementType::Int16 => Some("Int16Builder"),
                                    ElementType::Int32 => Some("Int32Builder"),
                                    ElementType::Int64 => Some("Int64Builder"),
                                    ElementType::Bool => Some("BoolBuilder"),
                                    ElementType::Float16 => Some("HalfFloatBuilder"),
                                    ElementType::Float32 => Some("FloatBuilder"),
                                    ElementType::Float64 => Some("DoubleBuilder"),
                                    ElementType::String => Some("StringBuilder"),
                                    ElementType::Object(_) => None,
                                };

                                if let Some(type_builder_name) = type_builder_name {
                                    let typ_builder_ident = format_ident!("{type_builder_name}");

                                    quote! {
                                        ARROW_RETURN_NOT_OK(variant_builder->Append());

                                        auto value_builder =
                                            static_cast<arrow::#typ_builder_ident *>(variant_builder->value_builder());
                                        ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                                            union_instance.get_union_data().#field_name.data(),
                                            static_cast<int64_t>(union_instance.get_union_data().#field_name.size())
                                        ));
                                    }
                                } else {
                                    let error = format!("Failed to serialize {}::{}: objects ({:?}) in unions not yet implemented", obj.name, variant.name, element_type);
                                    quote! {
                                        (void)#variant_builder;
                                        return rerun::Error(ErrorCode::NotImplemented, #error);
                                    }
                                }
                            }
                        } else {
                            let error = format!("Failed to serialize {}::{}: {} in unions not yet implemented", obj.name, variant.name, arrow_builder_type);
                            quote! {
                                (void)#variant_builder;
                                return rerun::Error(ErrorCode::NotImplemented, #error);
                            }
                        }
                    } else {
                        let variant_accessor = quote!(union_instance.get_union_data());
                        quote_append_single_field_to_builder(variant, &variant_builder, &variant_accessor, includes)
                    };

                    quote! {
                        case TagType::#variant_name: {
                            auto #variant_builder = static_cast<arrow::#arrow_builder_type*>(variant_builder_untyped);
                            #variant_append
                        } break;
                    }
                });

                let tag_name = format_ident!("{}Tag", type_ident);

                quote! {
                    #parameter_check
                    ARROW_RETURN_NOT_OK(#builder->Reserve(static_cast<int64_t>(num_elements)));
                    for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                        const auto& union_instance = elements[elem_idx];
                        ARROW_RETURN_NOT_OK(#builder->Append(static_cast<int8_t>(union_instance.get_union_tag())));
                        #NEWLINE_TOKEN
                        #NEWLINE_TOKEN
                        auto variant_index = static_cast<int>(union_instance.get_union_tag());
                        auto variant_builder_untyped = builder->child_builder(variant_index).get();
                        #NEWLINE_TOKEN
                        #NEWLINE_TOKEN

                        using TagType = #quoted_namespace::detail::#tag_name;

                        switch (union_instance.get_union_tag()) {
                            case TagType::None: {
                                ARROW_RETURN_NOT_OK(variant_builder_untyped->AppendNull());
                            } break;

                            #(#tag_cases)*

                            default:
                                assert(false && "unreachable");
                        }
                    }
                }
            }
        }
    }
}

fn quote_append_field_to_builder(
    field: &ObjectField,
    builder: &Ident,
    is_transparent: bool,
    includes: &mut Includes,
    objects: &Objects,
) -> TokenStream {
    let field_name = field_name_identifier(field);

    if let Some(elem_type) = field.typ.plural_inner() {
        let value_builder = format_ident!("value_builder");
        let value_builder_type = arrow_array_builder_type(&elem_type.clone().into(), objects);

        if !field.is_nullable
            && matches!(field.typ, Type::Array { .. })
            && elem_type.has_default_destructor(objects)
        {
            // Optimize common case: Trivial batch of transparent fixed size elements.
            let field_accessor = quote!(elements[0].#field_name);
            let num_items_per_value = quote_num_items_per_value(&field.typ, &field_accessor);
            quote! {
                auto #value_builder = static_cast<arrow::#value_builder_type*>(#builder->value_builder());
                #NEWLINE_TOKEN #NEWLINE_TOKEN
                ARROW_RETURN_NOT_OK(#builder->AppendValues(static_cast<int64_t>(num_elements)));
                static_assert(sizeof(elements[0].#field_name) == sizeof(elements[0]));
                ARROW_RETURN_NOT_OK(#value_builder->AppendValues(
                    #field_accessor.data(),
                    static_cast<int64_t>(num_elements * #num_items_per_value), nullptr)
                );
            }
        } else {
            let value_reserve_factor = match &field.typ {
                Type::Vector { .. } => {
                    if field.is_nullable {
                        1
                    } else {
                        2
                    }
                }
                Type::Array { length, .. } => *length,
                _ => unreachable!(),
            };
            let value_reserve_factor = quote_integer(value_reserve_factor);

            let setup = quote! {
                auto #value_builder = static_cast<arrow::#value_builder_type*>(#builder->value_builder());
                ARROW_RETURN_NOT_OK(#builder->Reserve(static_cast<int64_t>(num_elements)));
                ARROW_RETURN_NOT_OK(#value_builder->Reserve(static_cast<int64_t>(num_elements * #value_reserve_factor)));
                #NEWLINE_TOKEN #NEWLINE_TOKEN
            };

            let value_accessor = if field.is_nullable {
                quote!(element.#field_name.value())
            } else {
                quote!(element.#field_name)
            };

            let append_value = quote_append_single_value_to_builder(
                &field.typ,
                &value_builder,
                &value_accessor,
                includes,
            );

            if field.is_nullable {
                quote! {
                    #setup
                    for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                        const auto& element = elements[elem_idx];
                        if (element.#field_name.has_value()) {
                            ARROW_RETURN_NOT_OK(#builder->Append());
                            #append_value
                        } else {
                            ARROW_RETURN_NOT_OK(#builder->AppendNull());
                        }
                    }
                }
            } else {
                quote! {
                    #setup
                    for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                        const auto& element = elements[elem_idx];
                        ARROW_RETURN_NOT_OK(#builder->Append());
                        #append_value
                    }
                }
            }
        }
    } else if !field.is_nullable && is_transparent && field.typ.has_default_destructor(objects) {
        // Trivial optimization: If this is the only field of this type and it's a trivial field (not array/string/blob),
        // we can just pass the whole array as-is!
        let field_ptr_accessor = quote_field_ptr_access(&field.typ, &quote!(elements->#field_name));
        quote! {
            static_assert(sizeof(*elements) == sizeof(elements->#field_name));
            ARROW_RETURN_NOT_OK(#builder->AppendValues(#field_ptr_accessor, static_cast<int64_t>(num_elements)));
        }
    } else {
        let element_accessor = quote!(elements[elem_idx]);
        let single_append =
            quote_append_single_field_to_builder(field, builder, &element_accessor, includes);
        quote! {
            ARROW_RETURN_NOT_OK(#builder->Reserve(static_cast<int64_t>(num_elements)));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                #single_append
            }
        }
    }
}

fn quote_append_single_field_to_builder(
    field: &ObjectField,
    builder: &Ident,
    element_accessor: &TokenStream,
    includes: &mut Includes,
) -> TokenStream {
    let field_name = format_ident!("{}", field.snake_case_name());
    let value_access = if field.is_nullable {
        quote!(element.#field_name.value())
    } else {
        quote!(#element_accessor.#field_name)
    };

    let append_value =
        quote_append_single_value_to_builder(&field.typ, builder, &value_access, includes);

    if field.is_nullable {
        quote! {
            const auto& element = #element_accessor;
            if (element.#field_name.has_value()) {
                #append_value
            } else {
                ARROW_RETURN_NOT_OK(#builder->AppendNull());
            }
        }
    } else {
        quote! {
            #append_value
        }
    }
}

/// Appends a single value to an arrow array builder.
///
/// If the value is an array/vector, it will try to append the batch in one go.
/// Note that in that case this does *not* take care of the array/vector builder itself, just the underlying value builder.
fn quote_append_single_value_to_builder(
    typ: &Type,
    value_builder: &Ident,
    value_access: &TokenStream,
    includes: &mut Includes,
) -> TokenStream {
    match &typ {
        Type::Unit => {
            quote!(ARROW_RETURN_NOT_OK(#value_builder->AppendNull());)
        }

        Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::Bool
        | Type::Float32
        | Type::Float64
        | Type::String => {
            quote!(ARROW_RETURN_NOT_OK(#value_builder->Append(#value_access));)
        }
        Type::Float16 => {
            // Cast `rerun::half` to a `uint16_t``
            quote! {
                ARROW_RETURN_NOT_OK(#value_builder->Append(
                    *reinterpret_cast<const uint16_t*>(&(#value_access))
                ));
            }
        }
        Type::Array { elem_type, .. } | Type::Vector { elem_type } => {
            let num_items_per_element = quote_num_items_per_value(typ, value_access);

            match elem_type {
                ElementType::UInt8
                | ElementType::UInt16
                | ElementType::UInt32
                | ElementType::UInt64
                | ElementType::Int8
                | ElementType::Int16
                | ElementType::Int32
                | ElementType::Int64
                | ElementType::Bool
                | ElementType::Float32
                | ElementType::Float64 => {
                    let field_ptr_accessor = quote_field_ptr_access(typ, value_access);
                    quote! {
                        ARROW_RETURN_NOT_OK(#value_builder->AppendValues(#field_ptr_accessor, static_cast<int64_t>(#num_items_per_element), nullptr));
                    }
                }
                ElementType::Float16 => {
                    // We need to convert `rerun::half` to `uint16_t`:
                    let field_ptr_accessor = quote_field_ptr_access(typ, value_access);
                    quote! {
                        ARROW_RETURN_NOT_OK(#value_builder->AppendValues(
                            reinterpret_cast<const uint16_t*>(#field_ptr_accessor),
                            static_cast<int64_t>(#num_items_per_element), nullptr)
                        );
                    }
                }
                ElementType::String => {
                    quote! {
                        for (size_t item_idx = 0; item_idx < #num_items_per_element; item_idx += 1) {
                            ARROW_RETURN_NOT_OK(#value_builder->Append(#value_access[item_idx]));
                        }
                    }
                }
                ElementType::Object(fqname) => {
                    let fqname = quote_fqname_as_type_path(includes, fqname);
                    let field_ptr_accessor = quote_field_ptr_access(typ, value_access);
                    quote! {
                        if (#field_ptr_accessor) {
                            RR_RETURN_NOT_OK(Loggable<#fqname>::fill_arrow_array_builder(#value_builder, #field_ptr_accessor, #num_items_per_element));
                        }
                    }
                }
            }
        }
        Type::Object(fqname) => {
            let fqname = quote_fqname_as_type_path(includes, fqname);
            quote!(RR_RETURN_NOT_OK(Loggable<#fqname>::fill_arrow_array_builder(#value_builder, &#value_access, 1));)
        }
    }
}

fn quote_num_items_per_value(typ: &Type, value_accessor: &TokenStream) -> TokenStream {
    match &typ {
        Type::Array { length, .. } => quote_integer(length),
        Type::Vector { .. } => quote!(#value_accessor.size()),
        _ => quote_integer(1),
    }
}

fn quote_field_ptr_access(typ: &Type, field_accessor: &TokenStream) -> TokenStream {
    let (ptr_access, typ) = match typ {
        Type::Array { elem_type, .. } | Type::Vector { elem_type } => {
            (quote!(#field_accessor.data()), elem_type.clone().into())
        }
        _ => (quote!(&#field_accessor), typ.clone()),
    };

    if typ == Type::Bool {
        // Bool needs a cast because arrow takes it as uint8_t.
        quote!(reinterpret_cast<const uint8_t*>(#ptr_access))
    } else {
        ptr_access
    }
}

/// e.g. `static Angle radians(float radians);` -> `auto angle = Angle::radians(radians);`
fn static_constructor_for_enum_type(
    hpp_includes: &mut Includes,
    obj_field: &ObjectField,
    pascal_case_ident: &Ident,
    tag_typename: &Ident,
) -> Method {
    let tag_ident = format_ident!("{}", obj_field.name);
    // We don't use the `from_` prefix here, because this is instantiating an enum variant,
    // e.g. `Scale3D::Uniform(2.0)` in Rust becomes `Scale3D::uniform(2.0)` in C++.
    let function_name_ident = format_ident!("{}", obj_field.snake_case_name());
    let snake_case_ident = format_ident!("{}", obj_field.snake_case_name());
    let docs = obj_field.docs.clone().into();

    let param_declaration = quote_variable(hpp_includes, obj_field, &snake_case_ident);
    let declaration = MethodDeclaration {
        is_static: true,
        return_type: quote!(#pascal_case_ident),
        name_and_parameters: quote!(#function_name_ident(#param_declaration)),
    };

    let data_setter = if obj_field.typ == Type::Unit {
        quote! {}
    } else {
        // We need to use placement-new since the union is in an uninitialized state here:
        //
        // Do *not* assign (move _or_ copy).
        // At this point self._data is uninitialized, so only placement new is safe since we have to regard the target as "raw memory".
        // Otherwise we may call a function (move assignment or copy assignment)
        // on an uninitialized object which means that the compiler may optimize away the assignment.
        // (This was identified as the cause of #3865.)
        hpp_includes.insert_system("new"); // placement-new
        let typ = quote_field_type(hpp_includes, obj_field);
        quote! { new (&self._data.#snake_case_ident) #typ(std::move(#snake_case_ident)); }
    };

    Method {
        docs,
        declaration,
        definition_body: quote! {
            #pascal_case_ident self;
            self._tag = detail::#tag_typename::#tag_ident;
            #data_setter
            return self;
        },
        inline: true,
    }
}

fn are_types_disjoint(fields: &[ObjectField]) -> bool {
    let type_set: std::collections::HashSet<&Type> = fields.iter().map(|f| &f.typ).collect();
    type_set.len() == fields.len()
}

fn quote_archetype_field_type(hpp_includes: &mut Includes, obj_field: &ObjectField) -> TokenStream {
    match &obj_field.typ {
        Type::Vector { elem_type } => {
            hpp_includes.insert_rerun("collection.hpp");
            let elem_type = quote_element_type(hpp_includes, elem_type);
            quote! { Collection<#elem_type> }
        }
        // TODO(andreas): This should emit `MonoCollection` which will be a constrained version of `Collection`.
        // (Simply adapting `MonoCollection` breaks some existing code, so this not entirely trivial to do.
        //  Designing constraints for `MonoCollection` is harder still)
        Type::Object(fqname) => quote_fqname_as_type_path(hpp_includes, fqname),
        _ => panic!("Only vectors and objects are allowed in archetypes."),
    }
}

fn quote_variable_with_docstring(
    reporter: &Reporter,
    objects: &Objects,
    includes: &mut Includes,
    obj_field: &ObjectField,
    name: &syn::Ident,
) -> TokenStream {
    let quoted = quote_variable(includes, obj_field, name);

    let docstring = quote_field_docs(reporter, objects, obj_field);

    let quoted = quote! {
        #docstring
        #quoted
    };

    quoted
}

fn quote_field_type(includes: &mut Includes, obj_field: &ObjectField) -> TokenStream {
    #[allow(clippy::match_same_arms)]
    let typ = match &obj_field.typ {
        Type::Unit => panic!("Can't express the unit type directly"),

        Type::UInt8 => quote! { uint8_t  },
        Type::UInt16 => quote! { uint16_t  },
        Type::UInt32 => quote! { uint32_t  },
        Type::UInt64 => quote! { uint64_t  },
        Type::Int8 => quote! { int8_t  },
        Type::Int16 => quote! { int16_t  },
        Type::Int32 => quote! { int32_t  },
        Type::Int64 => quote! { int64_t  },
        Type::Bool => quote! { bool  },
        Type::Float16 => {
            includes.insert_rerun("half.hpp");
            quote! { rerun::half  }
        }
        Type::Float32 => quote! { float  },
        Type::Float64 => quote! { double  },
        Type::String => {
            includes.insert_system("string");
            quote! { std::string  }
        }
        Type::Array { elem_type, length } => {
            includes.insert_system("array");
            let elem_type = quote_element_type(includes, elem_type);
            let length = Literal::usize_unsuffixed(*length);
            quote! { std::array<#elem_type, #length> }
        }
        Type::Vector { elem_type } => {
            let elem_type = quote_element_type(includes, elem_type);
            includes.insert_rerun("collection.hpp");
            quote! { rerun::Collection<#elem_type>  }
        }
        Type::Object(fqname) => {
            let type_name = quote_fqname_as_type_path(includes, fqname);
            quote! { #type_name  }
        }
    };

    if obj_field.is_nullable {
        includes.insert_system("optional");
        quote! { std::optional<#typ> }
    } else {
        typ
    }
}

fn quote_variable(
    includes: &mut Includes,
    obj_field: &ObjectField,
    name: &syn::Ident,
) -> TokenStream {
    if obj_field.typ == Type::Unit {
        quote! {}
    } else {
        let typ = quote_field_type(includes, obj_field);
        quote! { #typ #name }
    }
}

fn quote_element_type(includes: &mut Includes, typ: &ElementType) -> TokenStream {
    #[allow(clippy::match_same_arms)]
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
        ElementType::Float16 => {
            includes.insert_rerun("half.hpp");
            quote! { rerun::half }
        }
        ElementType::Float32 => quote! { float },
        ElementType::Float64 => quote! { double },
        ElementType::String => {
            includes.insert_system("string");
            quote! { std::string }
        }
        ElementType::Object(fqname) => quote_fqname_as_type_path(includes, fqname),
    }
}

fn quote_fqname_as_type_path(includes: &mut Includes, fqname: &str) -> TokenStream {
    includes.insert_rerun_type(fqname);

    let fqname = fqname
        .replace(".testing", "")
        .replace('.', "::")
        .replace("crate", "rerun");

    let expr: syn::TypePath = syn::parse_str(&fqname).unwrap();
    quote!(#expr)
}

fn quote_obj_docs(reporter: &Reporter, objects: &Objects, obj: &Object) -> TokenStream {
    let mut lines = lines_from_docs(reporter, objects, &obj.docs);

    if let Some(first_line) = lines.first_mut() {
        // Prefix with object kind:
        *first_line = format!("**{}**: {}", obj.kind.singular_name(), first_line);
    }

    if obj.is_experimental() {
        lines.push(String::new());
        lines.push(
            "⚠ **This is an experimental API! It is not fully supported, and is likely to change significantly in future versions.**".to_owned(),
        );
    }

    quote_doc_lines(&lines)
}

fn quote_field_docs(reporter: &Reporter, objects: &Objects, field: &ObjectField) -> TokenStream {
    let lines = lines_from_docs(reporter, objects, &field.docs);
    quote_doc_lines(&lines)
}

fn lines_from_docs(reporter: &Reporter, objects: &Objects, docs: &Docs) -> Vec<String> {
    let mut lines = docs.lines_for(reporter, objects, Target::Cpp);

    let required = true;
    let examples = collect_snippets_for_api_docs(docs, "cpp", required).unwrap_or_default();
    if !examples.is_empty() {
        lines.push(String::new());
        let section_title = if examples.len() == 1 {
            "Example"
        } else {
            "Examples"
        };
        lines.push(format!("## {section_title}"));
        lines.push(String::new());
        let mut examples = examples.into_iter().peekable();
        while let Some(example) = examples.next() {
            let ExampleInfo {
                path,
                name,
                title,
                image,
                ..
            } = &example.base;

            for line in &example.lines {
                assert!(!line.contains("```"), "Example {path:?} contains ``` in it, so we can't embed it in the C++ API docs.");
            }

            if let Some(title) = title {
                lines.push(format!("### {title}"));
            } else {
                // Other languages put the name in backticks but doxygen doesn't support this on headings.
                lines.push(format!("### {name}:"));
            }

            if let Some(image) = image {
                match image {
                    super::common::ImageUrl::Rerun(image) => lines.push(image.markdown_tag()),
                    super::common::ImageUrl::Other(url) => {
                        lines.push(format!("![example image]({url})"));
                    }
                }
                lines.push(String::new());
            }

            lines.push("```cpp".into());
            lines.extend(example.lines.iter().cloned());
            lines.push("```".into());
            if examples.peek().is_some() {
                // blank line between examples
                lines.push(String::new());
            }
        }
    }

    lines
}

fn quote_doc_lines(lines: &[String]) -> TokenStream {
    let quoted_lines = lines.iter().map(|docstring| quote_doc_comment(docstring));
    quote! {
        #NEWLINE_TOKEN
        #(#quoted_lines)*
    }
}

fn quote_integer<T: std::fmt::Display>(t: T) -> TokenStream {
    let t = syn::LitInt::new(&t.to_string(), proc_macro2::Span::call_site());
    quote!(#t)
}

fn quote_arrow_datatype(
    typ: &Type,
    objects: &Objects,
    includes: &mut Includes,
    is_top_level_type: bool,
) -> TokenStream {
    match typ {
        Type::Unit => quote!(arrow::null()),
        Type::Int8 => quote!(arrow::int8()),
        Type::Int16 => quote!(arrow::int16()),
        Type::Int32 => quote!(arrow::int32()),
        Type::Int64 => quote!(arrow::int64()),
        Type::UInt8 => quote!(arrow::uint8()),
        Type::UInt16 => quote!(arrow::uint16()),
        Type::UInt32 => quote!(arrow::uint32()),
        Type::UInt64 => quote!(arrow::uint64()),
        Type::Float16 => quote!(arrow::float16()),
        Type::Float32 => quote!(arrow::float32()),
        Type::Float64 => quote!(arrow::float64()),
        Type::String => quote!(arrow::utf8()),
        Type::Bool => quote!(arrow::boolean()),

        Type::Vector { elem_type } => {
            let quoted_field = quote_arrow_elem_type(elem_type, objects, includes);
            quote!(arrow::list(#quoted_field))
        }

        Type::Array { elem_type, length } => {
            let quoted_field = quote_arrow_elem_type(elem_type, objects, includes);
            let quoted_length = quote_integer(length);
            quote!(arrow::fixed_size_list(#quoted_field, #quoted_length))
        }

        Type::Object(fqname) => {
            // TODO(andreas): We're no`t emitting the actual extension types here yet which is why we're skipping the extension type at top level.
            // Currently, we wrap only Components in extension types but this is done in `rerun_c`.
            // In the future we'll add the extension type here to the schema.
            let obj = &objects[fqname];
            if !is_top_level_type {
                // If we're not at the top level, we should have already a `arrow_datatype` method that we can relay to.
                let quoted_fqname = quote_fqname_as_type_path(includes, fqname);
                quote!(Loggable<#quoted_fqname>::arrow_datatype())
            } else if obj.is_arrow_transparent() {
                quote_arrow_datatype(&obj.fields[0].typ, objects, includes, false)
            } else {
                let quoted_fields = obj
                    .fields
                    .iter()
                    .map(|field| quote_arrow_field_type(field, objects, includes));

                match &obj.class {
                    ObjectClass::Struct => {
                        quote!(arrow::struct_({ #(#quoted_fields,)* }))
                    }
                    ObjectClass::Enum => {
                        quote! {
                            arrow::uint8()
                        }
                    }
                    ObjectClass::Union => {
                        quote! {
                            arrow::dense_union({
                                arrow::field("_null_markers", arrow::null(), true, nullptr),
                                #(#quoted_fields,)*
                            })
                        }
                    }
                }
            }
        }
    }
}

fn quote_arrow_field_type(
    field: &ObjectField,
    objects: &Objects,
    includes: &mut Includes,
) -> TokenStream {
    let name = &field.name;
    let datatype = quote_arrow_datatype(&field.typ, objects, includes, false);
    let is_nullable = field.is_nullable || field.typ == Type::Unit; // null type is always nullable
    let is_nullable = is_nullable || field.typ.is_union(objects); // Rerun unions always has a `_null_marker: null` variant, so they are always nullable

    quote! {
        arrow::field(#name, #datatype, #is_nullable)
    }
}

fn quote_arrow_elem_type(
    elem_type: &ElementType,
    objects: &Objects,
    includes: &mut Includes,
) -> TokenStream {
    let typ: Type = elem_type.clone().into();
    let datatype = quote_arrow_datatype(&typ, objects, includes, false);
    let is_nullable = typ == Type::Unit; // null type must be nullable
    let is_nullable = is_nullable || elem_type.is_union(objects); // Rerun unions always has a `_null_marker: null` variant, so they are always nullable
    quote! {
        arrow::field("item", #datatype, #is_nullable)
    }
}

fn quote_loggable_hpp_and_cpp(
    reporter: &Reporter,
    obj: &Object,
    objects: &Objects,
    hpp_includes: &mut Includes,
    cpp_includes: &mut Includes,
    hpp_declarations: &mut ForwardDecls,
) -> (TokenStream, Option<TokenStream>) {
    assert!(obj.kind != ObjectKind::Archetype);

    let namespace_ident = obj.namespace_ident();
    let type_ident = obj.ident();

    let quoted_namespace = if let Some(scope) = obj.scope() {
        let scope = format_ident!("{}", scope);
        quote! { #scope::#namespace_ident }
    } else {
        quote! {#namespace_ident}
    };

    let fqname = &obj.fqname;
    let loggable_type_name = quote! { Loggable<#quoted_namespace::#type_ident> };

    let mut methods = vec![
        arrow_data_type_method(obj, objects, hpp_includes, cpp_includes, hpp_declarations),
        to_arrow_method(obj, objects, hpp_includes, hpp_declarations),
    ];

    let predeclarations_and_static_assertions = if let Some(forwarded_fqname) =
        transparent_forwarded_fqname(obj)
    {
        // We only actually need `to_arrow`, everything else is just nice to have.

        let forwarded_type = quote_fqname_as_type_path(hpp_includes, forwarded_fqname);

        // Don't to pre-declare `Loggable` if we're forwarding to another type - it's already known in this case.
        quote! { static_assert(sizeof(#forwarded_type) == sizeof(#quoted_namespace::#type_ident)); }
    } else {
        // `fill_arrow_array_builder_method` is used as a utility to implement `to_arrow`.
        // We only need it if we're not forwarding to another type.
        methods.push(fill_arrow_array_builder_method(
            obj,
            cpp_includes,
            hpp_declarations,
            objects,
        ));

        quote! {
            // Instead of including loggable.hpp, simply re-declare the template since it's trivial
            template<typename T>
            struct Loggable;
        }
    };

    let methods_hpp = methods.iter().map(|m| m.to_hpp_tokens(reporter, objects));
    let methods_cpp = methods.iter().map(|m| m.to_cpp_tokens(&loggable_type_name));
    let hide_from_docs_comment = quote_hide_from_docs();

    hpp_includes.insert_rerun("component_descriptor.hpp");

    let (deprecation_ignore_start, deprecation_ignore_end) =
        quote_deprecation_ignore_start_and_end(hpp_includes, obj.deprecation_notice().is_some());

    let hpp = quote! {
        #deprecation_ignore_start

        namespace rerun {
            #predeclarations_and_static_assertions

            #hide_from_docs_comment
            template<>
            struct #loggable_type_name {
                static constexpr ComponentDescriptor Descriptor = #fqname;
                #NEWLINE_TOKEN
                #NEWLINE_TOKEN
                #(#methods_hpp)*
            };
        }

        #deprecation_ignore_end
    };

    let cpp = if methods.iter().any(|m| !m.inline) {
        Some(quote! {
            namespace rerun {
                #(#methods_cpp)*
            }
        })
    } else {
        None
    };

    (hpp, cpp)
}

fn quote_deprecation_notice(obj: &Object) -> TokenStream {
    if let Some(deprecation_notice) = obj.deprecation_notice() {
        // https://en.cppreference.com/w/cpp/language/attributes/deprecated
        quote! {
            [[deprecated(#deprecation_notice)]]
        }
    } else {
        quote! {}
    }
}

fn quote_deprecation_ignore_start_and_end(
    includes: &mut Includes,
    should_deprecate: bool,
) -> (TokenStream, TokenStream) {
    if should_deprecate {
        includes.insert_rerun("compiler_utils.hpp");
        (
            quote! {
                RR_PUSH_WARNINGS #NEWLINE_TOKEN
                RR_DISABLE_DEPRECATION_WARNING #NEWLINE_TOKEN
            },
            quote! { RR_POP_WARNINGS },
        )
    } else {
        (quote!(), quote!())
    }
}
