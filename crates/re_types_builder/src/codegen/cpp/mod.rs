mod forward_decl;
mod includes;
mod method;

use std::collections::BTreeSet;

use anyhow::Context;
use arrow2::datatypes::DataType;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use rayon::prelude::*;

use crate::codegen::common::write_file;
use crate::Object;
use crate::{
    codegen::AUTOGEN_WARNING, ArrowRegistry, Docs, ElementType, ObjectField, ObjectKind, Objects,
    Type,
};

use self::forward_decl::ForwardDecls;
use self::includes::Includes;
use self::method::{Method, MethodDeclaration};

// Special strings we insert as tokens, then search-and-replace later.
// This is so that we can insert comments and whitespace into the generated code.
// `TokenStream` ignores whitespace (including comments), but we can insert "quoted strings",
// so that is what we do.
const NEWLINE_TOKEN: &str = "NEWLINE_TOKEN";
const NORMAL_COMMENT_PREFIX_TOKEN: &str = "NORMAL_COMMENT_PREFIX_TOKEN";
const NORMAL_COMMENT_SUFFIX_TOKEN: &str = "NORMAL_COMMENT_SUFFIX_TOKEN";
const DOC_COMMENT_PREFIX_TOKEN: &str = "DOC_COMMENT_PREFIX_TOKEN";
const DOC_COMMENT_SUFFIX_TOKEN: &str = "DOC_COMMENT_SUFFIX_TOKEN";
const SYS_INCLUDE_PATH_PREFIX_TOKEN: &str = "SYS_INCLUDE_PATH_PREFIX_TOKEN";
const SYS_INCLUDE_PATH_SUFFIX_TOKEN: &str = "SYS_INCLUDE_PATH_SUFFIX_TOKEN";
const TODO_TOKEN: &str = "TODO_TOKEN";

fn quote_comment(text: &str) -> TokenStream {
    quote! { #NORMAL_COMMENT_PREFIX_TOKEN #text #NORMAL_COMMENT_SUFFIX_TOKEN }
}

fn quote_doc_comment(text: &str) -> TokenStream {
    quote! { #DOC_COMMENT_PREFIX_TOKEN #text #DOC_COMMENT_SUFFIX_TOKEN }
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
            .replace(&format!("{NORMAL_COMMENT_PREFIX_TOKEN:?} \""), "//")
            .replace(&format!("\" {NORMAL_COMMENT_SUFFIX_TOKEN:?}"), "\n")
            .replace(&format!("{DOC_COMMENT_PREFIX_TOKEN:?} \""), "///")
            .replace(&format!("\" {DOC_COMMENT_SUFFIX_TOKEN:?}"), "\n")
            .replace(&format!("{SYS_INCLUDE_PATH_PREFIX_TOKEN:?} \""), "<")
            .replace(&format!("\" {SYS_INCLUDE_PATH_SUFFIX_TOKEN:?}"), ">")
            .replace(
                &format!("{TODO_TOKEN:?}"),
                "\n// TODO(#2647): code-gen for C++\n",
            )
            .replace("< ", "<")
            .replace(" >", ">")
            .replace(" ::", "::"),
    );
    code.push('\n');

    // clang_format has a bit of an ugly API: https://github.com/KDAB/clang-format-rs/issues/3
    clang_format::CLANG_FORMAT_STYLE
        .set(clang_format::ClangFormatStyle::File)
        .ok();
    code = clang_format::clang_format(&code).expect("Failed to run clang-format");

    code
}

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
                #(#hash include #header_file_names "NEWLINE_TOKEN")*
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

fn generate_hpp_cpp(
    objects: &Objects,
    arrow_registry: &ArrowRegistry,
    obj: &Object,
) -> (TokenStream, TokenStream) {
    let QuotedObject { hpp, cpp } = QuotedObject::new(arrow_registry, objects, obj);
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
    pub fn new(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> Self {
        match obj.specifics {
            crate::ObjectSpecifics::Struct => Self::from_struct(arrow_registry, objects, obj),
            crate::ObjectSpecifics::Union { .. } => Self::from_union(arrow_registry, objects, obj),
        }
    }

    fn from_struct(
        arrow_registry: &ArrowRegistry,
        _objects: &Objects,
        obj: &Object,
    ) -> QuotedObject {
        let namespace_ident = format_ident!("{}", obj.kind.plural_snake_case()); // `datatypes`, `components`, or `archetypes`
        let pascal_case_name = &obj.name;
        let pascal_case_ident = format_ident!("{pascal_case_name}"); // The PascalCase name of the object type.
        let quoted_docs = quote_docstrings(&obj.docs);

        let mut hpp_includes = Includes::default();
        hpp_includes.system.insert("cstdint".to_owned()); // we use `uint32_t` etc everywhere.

        // Doing our own forward declarations doesn't get us super far since some arrow types like `FloatBuilder` are type aliases.
        // TODO(andreas): This drags in arrow headers into the public api though. We probably should try harder with forward declarations.
        hpp_includes.system.insert("arrow/type_fwd.h".to_owned());
        let mut cpp_includes = Includes::default();
        #[allow(unused)]
        let mut hpp_declarations = ForwardDecls::default();

        let field_declarations = obj
            .fields
            .iter()
            .map(|obj_field| {
                let declaration = quote_declaration_with_docstring(
                    &mut hpp_includes,
                    obj_field,
                    &format_ident!("{}", obj_field.name),
                );
                quote! {
                    #NEWLINE_TOKEN
                    #declaration
                }
            })
            .collect_vec();

        let mut methods = Vec::new();

        if obj.fields.len() == 1 {
            // Single-field struct - it is a newtype wrapper.
            // Create a implicit constructor from its own field-type.
            let obj_field = &obj.fields[0];
            if let Type::Array { .. } = &obj_field.typ {
                // TODO(emilk): implicit constructor for arrays
            } else {
                hpp_includes.system.insert("utility".to_owned()); // std::move

                let field_ident = format_ident!("{}", obj_field.name);
                let parameter_declaration =
                    quote_declaration(&mut hpp_includes, obj_field, &field_ident);

                methods.push(Method {
                    declaration: MethodDeclaration::constructor(quote! {
                        #pascal_case_ident(#parameter_declaration) : #field_ident(std::move(#field_ident))
                    }),
                    ..Method::default()
                });
            }
        };

        let datatype = arrow_registry.get(&obj.fqname);

        match obj.kind {
            ObjectKind::Datatype | ObjectKind::Component => {
                methods.push(arrow_data_type_method(&datatype, &mut cpp_includes));
                methods.push(new_arrow_array_builder_method(&datatype, &mut cpp_includes));
                methods.push(fill_arrow_array_builder_method(
                    &datatype,
                    obj,
                    &pascal_case_ident,
                    &mut cpp_includes,
                ));
            }
            ObjectKind::Archetype => {
                // TODO(andreas): Should also be convertable to arrow?
            }
        };

        let hpp_method_section = if methods.is_empty() {
            quote! {}
        } else {
            let hpp_methods = methods.iter().map(|m| m.to_hpp_tokens());
            quote! {
                public:
                    #(#hpp_methods)*
            }
        };
        let hpp = quote! {
            #hpp_includes

            #hpp_declarations

            namespace rr {
                namespace #namespace_ident {
                    #quoted_docs
                    struct #pascal_case_ident {
                        #(#field_declarations;)*
                        #hpp_method_section
                    };
                }
            }
        };

        let cpp_methods = methods.iter().map(|m| m.to_cpp_tokens(&pascal_case_ident));
        let cpp = quote! {
            #cpp_includes

            namespace rr {
                namespace #namespace_ident {
                    #(#cpp_methods)*
                }
            }
        };

        Self { hpp, cpp }
    }

    fn from_union(arrow_registry: &ArrowRegistry, objects: &Objects, obj: &Object) -> QuotedObject {
        // We implement sum-types as tagged unions;
        // Putting non-POD types in a union requires C++11.
        //
        // enum class Rotation3DTag {
        //     NONE = 0,
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

        let namespace_ident = format_ident!("{}", obj.kind.plural_snake_case()); // `datatypes`, `components`, or `archetypes`
        let pascal_case_name = &obj.name;
        let pascal_case_ident = format_ident!("{pascal_case_name}"); // The PascalCase name of the object type.
        let quoted_docs = quote_docstrings(&obj.docs);

        let tag_typename = format_ident!("{pascal_case_name}Tag");
        let data_typename = format_ident!("{pascal_case_name}Data");

        let tag_fields = std::iter::once({
            let comment = quote_doc_comment(
                "Having a special empty state makes it possible to implement move-semantics. \
                We need to be able to leave the object in a state which we can run the destructor on.");
            let tag_name = format_ident!("NONE");
            quote! {
                #NEWLINE_TOKEN
                #comment
                #tag_name = 0,
            }
        })
        .chain(obj.fields.iter().map(|obj_field| {
            let ident = format_ident!("{}", obj_field.name);
            quote! {
                #ident,
            }
        }))
        .collect_vec();

        let mut hpp_includes = Includes::default();
        hpp_includes.system.insert("cstdint".to_owned()); // we use `uint32_t` etc everywhere.
        hpp_includes.system.insert("utility".to_owned()); // std::move
        hpp_includes.system.insert("cstring".to_owned()); // std::memcpy

        // Doing our own forward declarations doesn't get us super far since some arrow types like `FloatBuilder` are type aliases.
        // TODO(andreas): This drags in arrow headers into the public api though. We probably should try harder with f
        hpp_includes.system.insert("arrow/type_fwd.h".to_owned());

        let mut cpp_includes = Includes::default();
        #[allow(unused)]
        let mut hpp_declarations = ForwardDecls::default();

        let enum_data_declarations = obj
            .fields
            .iter()
            .map(|obj_field| {
                let declaration = quote_declaration_with_docstring(
                    &mut hpp_includes,
                    obj_field,
                    &format_ident!("{}", crate::to_snake_case(&obj_field.name)),
                );
                quote! {
                    #NEWLINE_TOKEN
                    #declaration
                }
            })
            .collect_vec();

        let mut methods = Vec::new();

        // Add one static constructor for every field.
        for obj_field in &obj.fields {
            methods.push(static_constructor_for_enum_type(
                objects,
                &mut hpp_includes,
                obj_field,
                &pascal_case_ident,
                &tag_typename,
            ));
        }

        if are_types_disjoint(&obj.fields) {
            // Implicit construct from the different variant types:
            for obj_field in &obj.fields {
                let snake_case_ident = format_ident!("{}", crate::to_snake_case(&obj_field.name));
                let param_declaration =
                    quote_declaration(&mut hpp_includes, obj_field, &snake_case_ident);

                methods.push(Method {
                    docs: obj_field.docs.clone().into(),
                    declaration: MethodDeclaration::constructor(quote!(#pascal_case_ident(#param_declaration))),
                    definition_body: quote!(*this = #pascal_case_ident::#snake_case_ident(std::move(#snake_case_ident));),
                    inline: true,
                });
            }
        } else {
            // Cannot make implicit constructors, e.g. for
            // `enum Angle { Radians(f32), Degrees(f32) };`
        };

        let datatype = arrow_registry.get(&obj.fqname);
        methods.push(arrow_data_type_method(&datatype, &mut cpp_includes));
        methods.push(new_arrow_array_builder_method(&datatype, &mut cpp_includes));
        methods.push(fill_arrow_array_builder_method(
            &datatype,
            obj,
            &pascal_case_ident,
            &mut cpp_includes,
        ));

        let destructor = if obj.has_default_destructor(objects) {
            // No destructor needed
            quote! {}
        } else {
            let destructor_match_arms = std::iter::once({
                let comment = quote_comment("Nothing to destroy");
                quote! {
                    case detail::#tag_typename::NONE: {
                        break; #comment
                    }
                }
            })
            .chain(obj.fields.iter().map(|obj_field| {
                let tag_ident = format_ident!("{}", obj_field.name);
                let field_ident = format_ident!("{}", crate::to_snake_case(&obj_field.name));

                if obj_field.typ.has_default_destructor(objects) {
                    let comment = quote_comment("has a trivial destructor");
                    quote! {
                        case detail::#tag_typename::#tag_ident: {
                            break; #comment
                        }
                    }
                } else if let Type::Array { elem_type, length } = &obj_field.typ {
                    // We need special casing for destroying arrays in C++:
                    let elem_type = quote_element_type(&mut hpp_includes, elem_type);
                    let length = proc_macro2::Literal::usize_unsuffixed(*length);
                    quote! {
                        case detail::#tag_typename::#tag_ident: {
                            typedef #elem_type TypeAlias;
                            for (size_t i = #length; i > 0; i -= 1) {
                                _data.#field_ident[i-1].~TypeAlias();
                            }
                            break;
                        }
                    }
                } else {
                    let typedef_declaration = quote_declaration(
                        &mut hpp_includes,
                        obj_field,
                        &format_ident!("TypeAlias"),
                    );
                    hpp_includes.system.insert("utility".to_owned()); // std::move
                    quote! {
                        case detail::#tag_typename::#tag_ident: {
                            typedef #typedef_declaration;
                            _data.#field_ident.~TypeAlias();
                            break;
                        }
                    }
                }
            }))
            .collect_vec();

            quote! {
                ~#pascal_case_ident() {
                    switch (this->_tag) {
                        #(#destructor_match_arms)*
                    }
                }
            }
        };

        let swap_comment = quote_comment("This bitwise swap would fail for self-referential types, but we don't have any of those.");

        let hpp_methods = methods.iter().map(|m| m.to_hpp_tokens());
        let hpp = quote! {
            #hpp_includes

            #hpp_declarations

            namespace rr {
                namespace #namespace_ident {
                    namespace detail {
                        enum class #tag_typename {
                            #(#tag_fields)*
                        };

                        union #data_typename {
                            #(#enum_data_declarations;)*

                            #data_typename() { } // Required by static constructors
                            ~#data_typename() { }

                            void swap(#data_typename& other) noexcept {
                                #NEWLINE_TOKEN
                                #swap_comment
                                char temp[sizeof(#data_typename)];
                                std::memcpy(temp, this, sizeof(#data_typename));
                                std::memcpy(this, &other, sizeof(#data_typename));
                                std::memcpy(&other, temp, sizeof(#data_typename));
                            }
                        };

                    }

                    #quoted_docs
                    struct #pascal_case_ident {
                      private:
                        detail::#tag_typename _tag;
                        detail::#data_typename _data;

                        // Empty state required by static constructors:
                        #pascal_case_ident() : _tag(detail::#tag_typename::NONE) {}

                      public:
                        // Move-constructor:
                        #pascal_case_ident(#pascal_case_ident&& other) noexcept : _tag(detail::#tag_typename::NONE) {
                            this->swap(other);
                        }

                        // Move-assignment:
                        #pascal_case_ident& operator=(#pascal_case_ident&& other) noexcept {
                            this->swap(other);
                            return *this;
                        }

                        #destructor

                        #(#hpp_methods)*

                        // This is useful for easily implementing the move constructor and move assignment operator:
                        void swap(#pascal_case_ident& other) noexcept {
                            // Swap tags:
                            auto tag_temp = this->_tag;
                            this->_tag = other._tag;
                            other._tag = tag_temp;

                            // Swap data:
                            this->_data.swap(other._data);
                        }
                    };
                }
            }
        };

        let cpp_methods = methods.iter().map(|m| m.to_cpp_tokens(&pascal_case_ident));
        let cpp = quote! {
            #cpp_includes

            namespace rr {
                namespace #namespace_ident {
                    #(#cpp_methods)*
                }
            }
        };

        Self { hpp, cpp }
    }
}

fn arrow_data_type_method(datatype: &DataType, cpp_includes: &mut Includes) -> Method {
    cpp_includes.system.insert("arrow/api.h".to_owned());

    let quoted_datatype = quote_arrow_data_type(datatype, cpp_includes, true);

    Method {
        docs: "Returns the arrow data type this type corresponds to.".into(),
        declaration: MethodDeclaration {
            is_static: true,
            return_type: quote! { std::shared_ptr<arrow::DataType> },
            name_and_parameters: quote! { to_arrow_datatype() },
        },
        definition_body: quote! { return #quoted_datatype; },
        inline: false,
    }
}

fn new_arrow_array_builder_method(datatype: &DataType, cpp_includes: &mut Includes) -> Method {
    let arrow_builder_type = arrow_array_builder_type(datatype);
    let arrow_builder_type = format_ident!("{arrow_builder_type}");
    let builder_instantiation =
        quote_arrow_array_builder_type_instantiation(datatype, cpp_includes, true);

    Method {
        docs: "Creates a new array builder with an array of this type.".into(),
        declaration: MethodDeclaration {
            is_static: true,
            return_type: quote! { arrow::Result<std::shared_ptr<arrow::#arrow_builder_type>> },
            name_and_parameters: quote!(new_arrow_array_builder(arrow::MemoryPool * memory_pool)),
        },
        definition_body: quote! {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
            return arrow::Result(#builder_instantiation);
        },
        inline: false,
    }
}

fn fill_arrow_array_builder_method(
    datatype: &DataType,
    obj: &Object,
    pascal_case_ident: &Ident,
    cpp_includes: &mut Includes,
) -> Method {
    let DataType::Extension(_fqname, logical_datatype, _metadata) = datatype else {
        panic!("Can only generate arrow serialization code for extension types.");
    };

    let builder = format_ident!("builder");
    let arrow_builder_type = arrow_array_builder_type(datatype);
    let arrow_builder_type = format_ident!("{arrow_builder_type}");

    let fill_builder = quote_fill_arrow_array_builder(
        pascal_case_ident,
        logical_datatype,
        &obj.fields,
        &builder,
        cpp_includes,
    )
    .context(format!("Generating serialization for {}", obj.fqname))
    .unwrap();

    Method {
        docs: "Fills an arrow array builder with an array of this type.".into(),
        declaration: MethodDeclaration {
            is_static: true,
            return_type: quote! { arrow::Status },
            // TODO(andreas): Pass in validity map.
            name_and_parameters: quote! {
                fill_arrow_array_builder(arrow::#arrow_builder_type* #builder, const #pascal_case_ident* elements, size_t num_elements)
            },
        },
        definition_body: quote! {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
            #fill_builder
            #NEWLINE_TOKEN
            #NEWLINE_TOKEN
            return arrow::Status::OK();
        },
        inline: false,
    }
}

fn quote_fill_arrow_array_builder(
    pascal_case_ident: &Ident,
    datatype: &DataType,
    fields: &[ObjectField],
    builder: &Ident,
    cpp_includes: &mut Includes,
) -> anyhow::Result<TokenStream> {
    let tokens = match datatype {
        DataType::Boolean
        | DataType::Int8
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
        | DataType::Binary
        | DataType::LargeBinary
        | DataType::Utf8
        | DataType::LargeUtf8 => {
            anyhow::ensure!(
                fields.len() == 1,
                "Expected exactly one field for primitive type {:?}",
                datatype
            );
            let field = &fields[0];
            quote_append_elements_to_builder(field, builder, true)
        }
        DataType::FixedSizeList(field, length) => {
            anyhow::ensure!(
                fields.len() == 1,
                "Expected exactly one field for list type {:?}",
                datatype
            );
            match field.data_type() {
                DataType::Boolean
                | DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
                | DataType::Float16
                | DataType::Float32
                | DataType::Float64 => {}
                _ => anyhow::bail!("Unimplemented field type for fixed size list: {:?}", field),
            };

            let value_builder_type = arrow_array_builder_type(field.data_type());
            let value_builder_type = format_ident!("{value_builder_type}");
            let field_name = format_ident!("{}", &fields[0].name);
            let length = quote_integer(length);

            if field.is_nullable {
                quote! {
                    auto value_builder = static_cast<arrow::#value_builder_type*>(#builder->value_builder());
                    ARROW_RETURN_NOT_OK(#builder->Reserve(num_elements * #length));
                    ARROW_RETURN_NOT_OK(value_builder->Reserve(num_elements * #length));
                    #NEWLINE_TOKEN #NEWLINE_TOKEN
                    for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                        const auto& element = elements[elem_idx];
                        if (element.#field_name.has_value()) {
                            value_builder->AppendValues(element.#field_name.value(), #length, nullptr);
                            ARROW_RETURN_NOT_OK(#builder->Append());
                        } else {
                            ARROW_RETURN_NOT_OK(#builder->AppendNull());
                        }
                    }
                }
            } else {
                // TODO(andreas): Optimize for fully transparent types to do a single AppendValues call on value_builder.
                quote! {
                    auto value_builder = static_cast<arrow::#value_builder_type*>(#builder->value_builder());
                    #NEWLINE_TOKEN #NEWLINE_TOKEN
                    static_assert(sizeof(elements[0].#field_name) == sizeof(elements[0]));
                    ARROW_RETURN_NOT_OK(value_builder->AppendValues(elements[0].#field_name, num_elements * #length, nullptr));
                    ARROW_RETURN_NOT_OK(#builder->AppendValues(num_elements));
                }
            }
        }
        DataType::List(_field) => {
            quote!(
                return arrow::Status::NotImplemented(
                    "TODO(andreas): dynamically sized lists are not yet supported"
                );
            )
        }
        DataType::Struct(field_datatypes) => {
            let fill_fields = fields.iter().zip(field_datatypes).enumerate().map(
                |(i, (field, arrow_field))| {
                    let builder_index = quote_integer(i);
                    match arrow_field.data_type() {
                        DataType::FixedSizeList(..) | DataType::List(..) => {
                            quote!(
                                return arrow::Status::NotImplemented(
                                    "TODO(andreas): lists in structs are not yet supported"
                                );
                            )
                        }
                        DataType::Extension(_fqname, _, _) => {
                            quote!(
                                return arrow::Status::NotImplemented(
                                    "TODO(andreas): extensions in structs are not yet supported"
                                );
                            )
                        }
                        _ => {
                            let element_builder = format_ident!("element_builder");
                            let element_builder_type = arrow_array_builder_type(arrow_field.data_type());
                            let element_builder_type = format_ident!("{element_builder_type}");
                            let field_append = quote_append_elements_to_builder(field, &element_builder, false);
                            quote! {
                                {
                                    auto #element_builder = static_cast<arrow::#element_builder_type*>(builder->field_builder(#builder_index));
                                    #field_append
                                }
                            }
                        }
                    }
                },
            );

            quote! {
                #(#fill_fields)*
                #NEWLINE_TOKEN
                ARROW_RETURN_NOT_OK(builder->AppendValues(num_elements, nullptr));
            }
        }
        DataType::Union(_, _, _) => {
            quote! {
                #NEWLINE_TOKEN
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    const auto& element = elements[elem_idx];
                }
                return arrow::Status::NotImplemented(
                    "TODO(andreas): unions are not yet implemented"
                );
            }
        }
        DataType::Extension(fqname, _datatype, _) => {
            assert_eq!(fields.len(), 1);
            if fields[0].is_nullable {
                // Idea: pass in a tagged union for both optional and non-optional arrays to all fill_arrow_array_builder methods?
                quote!(return arrow::Status::NotImplemented(("TODO(andreas) Handle nullable extensions"));)
            } else {
                let quoted_fqname = quote_fqname_as_type_path(cpp_includes, fqname);
                quote! {
                    static_assert(sizeof(#quoted_fqname) == sizeof(#pascal_case_ident));
                    ARROW_RETURN_NOT_OK(#quoted_fqname::fill_arrow_array_builder(
                        builder, reinterpret_cast<const #quoted_fqname*>(elements), num_elements
                    ));
                }
            }
        }
        _ => anyhow::bail!(
            "Arrow serialization for type {:?} not implemented",
            datatype
        ),
    };

    Ok(tokens)
}

fn quote_append_elements_to_builder(
    field: &ObjectField,
    builder: &Ident,
    is_transparent: bool,
) -> TokenStream {
    let field_name = format_ident!("{}", field.name);
    if field.is_nullable {
        quote! {
            ARROW_RETURN_NOT_OK(#builder->Reserve(num_elements));
            for (auto elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                if (element.#field_name.has_value()) {
                    ARROW_RETURN_NOT_OK(#builder->Append(element.#field_name.value()));
                } else {
                    ARROW_RETURN_NOT_OK(#builder->AppendNull());
                }
            }
        }
    } else if is_transparent
        && !matches!(
            &field.typ,
            Type::String | Type::Vector { .. } | Type::Object(..) | Type::Array { .. }
        )
    {
        // Trivial optimization: If this is the only field of this type and it's a trivial field (not array/string/blob),
        // we can just pass the whole array as-is!
        quote! {
            static_assert(sizeof(*elements) == sizeof(elements->#field_name));
            ARROW_RETURN_NOT_OK(#builder->AppendValues(&elements->#field_name, num_elements));
        }
    } else {
        quote! {
            ARROW_RETURN_NOT_OK(#builder->Reserve(num_elements));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                ARROW_RETURN_NOT_OK(#builder->Append(elements[elem_idx].#field_name));
            }
        }
    }
}

fn arrow_array_builder_type(datatype: &DataType) -> &'static str {
    match datatype.to_logical_type() {
        DataType::Boolean => "BooleanBuilder",
        DataType::Int8 => "Int8Builder",
        DataType::Int16 => "Int16Builder",
        DataType::Int32 => "Int32Builder",
        DataType::Int64 => "Int64Builder",
        DataType::UInt8 => "UInt8Builder",
        DataType::UInt16 => "UInt16Builder",
        DataType::UInt32 => "UInt32Builder",
        DataType::UInt64 => "UInt64Builder",
        DataType::Float16 => "HalfFloatBuilder",
        DataType::Float32 => "FloatBuilder",
        DataType::Float64 => "DoubleBuilder",
        DataType::Binary => "BinaryBuilder",
        DataType::LargeBinary => "LargeBinaryBuilder",
        DataType::Utf8 => "StringBuilder",
        DataType::LargeUtf8 => "LargeStringBuilder",
        DataType::FixedSizeList(..) => "FixedSizeListBuilder",
        DataType::List(..) => "ListBuilder",
        DataType::Struct(..) => "StructBuilder",
        DataType::Null => "NullBuilder",
        DataType::Union(_, _, mode) => match mode {
            arrow2::datatypes::UnionMode::Dense => "DenseUnionBuilder",
            arrow2::datatypes::UnionMode::Sparse => "SparseUnionBuilder",
        },
        DataType::Extension(_, _, _metadata) => {
            unreachable!("Logical type can't be an extension type.")
        }
        _ => unimplemented!(
            "Arrow serialization for type {:?} not implemented",
            datatype
        ),
    }
}

fn quote_arrow_array_builder_type_instantiation(
    datatype: &DataType,
    cpp_includes: &mut Includes,
    is_top_level_type: bool,
) -> TokenStream {
    let builder_type = arrow_array_builder_type(datatype);
    let builder_type = format_ident!("{builder_type}");

    match datatype {
        DataType::Boolean
        | DataType::Int8
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
        | DataType::Binary
        | DataType::LargeBinary
        | DataType::Utf8
        | DataType::LargeUtf8
        | DataType::Null => {
            quote!(std::make_shared<arrow::#builder_type>(memory_pool))
        }
        DataType::List(field) => {
            let element_builder = quote_arrow_array_builder_type_instantiation(
                field.data_type(),
                cpp_includes,
                false,
            );
            quote!(std::make_shared<arrow::#builder_type>(memory_pool, #element_builder))
        }
        DataType::FixedSizeList(field, length) => {
            let quoted_length = quote_integer(length);
            let element_builder = quote_arrow_array_builder_type_instantiation(
                field.data_type(),
                cpp_includes,
                false,
            );
            quote!(std::make_shared<arrow::#builder_type>(memory_pool, #element_builder, #quoted_length))
        }
        DataType::Struct(fields) => {
            let field_builders = fields.iter().map(|field| {
                quote_arrow_array_builder_type_instantiation(field.data_type(), cpp_includes, false)
            });
            quote! {
                std::make_shared<arrow::#builder_type>(
                    to_arrow_datatype(),
                    memory_pool,
                    std::vector<std::shared_ptr<arrow::ArrayBuilder>>({ #(#field_builders,)* })
                )
            }
        }
        DataType::Union(fields, _, _) => {
            let field_builders = fields.iter().map(|field| {
                quote_arrow_array_builder_type_instantiation(field.data_type(), cpp_includes, false)
            });
            quote! {
                std::make_shared<arrow::#builder_type>(
                    memory_pool,
                    std::vector<std::shared_ptr<arrow::ArrayBuilder>>({ #(#field_builders,)* }),
                    to_arrow_datatype()
                )
            }
        }
        DataType::Extension(fqname, datatype, _metadata) => {
            if is_top_level_type {
                quote_arrow_array_builder_type_instantiation(datatype.as_ref(), cpp_includes, false)
            } else {
                // Propagating error here is hard since we're in a nested context.
                // But also not that important since we *know* that this only fails for null pools and we already checked that now.
                let quoted_fqname = quote_fqname_as_type_path(cpp_includes, fqname);
                quote! {
                    #quoted_fqname::new_arrow_array_builder(memory_pool).ValueOrDie()
                }
            }
        }
        _ => unimplemented!(
            "Arrow serialization for type {:?} not implemented",
            datatype
        ),
    }
}

/// e.g. `static Angle radians(float radians);` -> `auto angle = Angle::radians(radians);`
fn static_constructor_for_enum_type(
    objects: &Objects,
    hpp_includes: &mut Includes,
    obj_field: &ObjectField,
    pascal_case_ident: &Ident,
    tag_typename: &Ident,
) -> Method {
    let tag_ident = format_ident!("{}", obj_field.name);
    let snake_case_ident = format_ident!("{}", crate::to_snake_case(&obj_field.name));
    let docs = obj_field.docs.clone().into();

    let param_declaration = quote_declaration(hpp_includes, obj_field, &snake_case_ident);
    let declaration = MethodDeclaration {
        is_static: true,
        return_type: quote!(#pascal_case_ident),
        name_and_parameters: quote!(#snake_case_ident(#param_declaration)),
    };

    if let Type::Array { elem_type, length } = &obj_field.typ {
        // We need special casing for constructing arrays:
        let length = proc_macro2::Literal::usize_unsuffixed(*length);

        let element_assignment = if elem_type.has_default_destructor(objects) {
            // Generate simpoler code for simple types:
            quote! {
                self._data.#snake_case_ident[i] = std::move(#snake_case_ident[i]);
            }
        } else {
            // We need to use placement-new since the union is in an uninitialized state here:
            hpp_includes.system.insert("new".to_owned()); // placement-new
            quote! {
                new (&self._data.#snake_case_ident[i]) TypeAlias(std::move(#snake_case_ident[i]));
            }
        };

        let elem_type = quote_element_type(hpp_includes, elem_type);

        Method {
            docs,
            declaration,
            definition_body: quote! {
                typedef #elem_type TypeAlias;
                #pascal_case_ident self;
                self._tag = detail::#tag_typename::#tag_ident;
                for (size_t i = 0; i < #length; i += 1) {
                    #element_assignment
                }
                return std::move(self);
            },
            inline: true,
        }
    } else if obj_field.typ.has_default_destructor(objects) {
        // Generate simpler code for simple types:
        Method {
            docs,
            declaration,
            definition_body: quote! {
                #pascal_case_ident self;
                self._tag = detail::#tag_typename::#tag_ident;
                self._data.#snake_case_ident = std::move(#snake_case_ident);
                return std::move(self);
            },
            inline: true,
        }
    } else {
        // We need to use placement-new since the union is in an uninitialized state here:
        hpp_includes.system.insert("new".to_owned()); // placement-new
        let typedef_declaration =
            quote_declaration(hpp_includes, obj_field, &format_ident!("TypeAlias"));
        Method {
            docs,
            declaration,
            definition_body: quote! {
                typedef #typedef_declaration;
                #pascal_case_ident self;
                self._tag = detail::#tag_typename::#tag_ident;
                new (&self._data.#snake_case_ident) TypeAlias(std::move(#snake_case_ident));
                return std::move(self);
            },
            inline: true,
        }
    }
}

fn are_types_disjoint(fields: &[ObjectField]) -> bool {
    let type_set: std::collections::HashSet<&Type> = fields.iter().map(|f| &f.typ).collect();
    type_set.len() == fields.len()
}

fn quote_declaration_with_docstring(
    includes: &mut Includes,
    obj_field: &ObjectField,
    name: &syn::Ident,
) -> TokenStream {
    let quoted = quote_declaration(includes, obj_field, name);

    let docstring = quote_docstrings(&obj_field.docs);

    let quoted = quote! {
        #docstring
        #quoted
    };

    quoted
}

fn quote_declaration(
    includes: &mut Includes,
    obj_field: &ObjectField,
    name: &syn::Ident,
) -> TokenStream {
    if obj_field.is_nullable {
        includes.system.insert("optional".to_owned());
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
            Type::Float16 => unimplemented!("float16 not yet implemented for C++"),
            Type::Float32 => quote! { std::optional<float> #name },
            Type::Float64 => quote! { std::optional<double> #name },
            Type::String => {
                includes.system.insert("string".to_owned());
                quote! { std::optional<std::string> #name }
            }
            Type::Array { .. } => {
                unimplemented!(
                    "Optional fixed-size array not yet implemented in C++. {:#?}",
                    obj_field.typ
                )
            }
            Type::Vector { elem_type } => {
                let elem_type = quote_element_type(includes, elem_type);
                includes.system.insert("vector".to_owned());
                quote! { std::optional<std::vector<#elem_type>> #name }
            }
            Type::Object(fqname) => {
                let type_name = quote_fqname_as_type_path(includes, fqname);
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
            Type::Float16 => unimplemented!("float16 not yet implemented for C++"),
            Type::Float32 => quote! { float #name },
            Type::Float64 => quote! { double #name },
            Type::String => {
                includes.system.insert("string".to_owned());
                quote! { std::string #name }
            }
            Type::Array { elem_type, length } => {
                let elem_type = quote_element_type(includes, elem_type);
                let length = proc_macro2::Literal::usize_unsuffixed(*length);

                quote! { #elem_type #name[#length] }
            }
            Type::Vector { elem_type } => {
                let elem_type = quote_element_type(includes, elem_type);
                includes.system.insert("vector".to_owned());
                quote! { std::vector<#elem_type> #name }
            }
            Type::Object(fqname) => {
                let type_name = quote_fqname_as_type_path(includes, fqname);
                quote! { #type_name #name }
            }
        }
    }
}

fn quote_element_type(includes: &mut Includes, typ: &ElementType) -> TokenStream {
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
        ElementType::Float16 => unimplemented!("float16 not yet implemented for C++"),
        ElementType::Float32 => quote! { float },
        ElementType::Float64 => quote! { double },
        ElementType::String => {
            includes.system.insert("string".to_owned());
            quote! { std::string }
        }
        ElementType::Object(fqname) => quote_fqname_as_type_path(includes, fqname),
    }
}

fn quote_fqname_as_type_path(includes: &mut Includes, fqname: &str) -> TokenStream {
    let fqname = fqname
        .replace(".testing", "")
        .replace('.', "::")
        .replace("crate", "rr")
        .replace("rerun", "rr");

    // fqname example: "rr::datatypes::Transform3D"
    let components = fqname.split("::").collect::<Vec<_>>();
    if let ["rr", obj_kind, typname] = &components[..] {
        includes.local.insert(format!(
            "../{obj_kind}/{}.hpp",
            crate::to_snake_case(typname)
        ));
    }

    let expr: syn::TypePath = syn::parse_str(&fqname).unwrap();
    quote!(#expr)
}

fn quote_docstrings(docs: &Docs) -> TokenStream {
    let lines = crate::codegen::get_documentation(docs, &["cpp", "c++"]);
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

// --- Arrow registry code generators ---

fn quote_arrow_data_type(
    datatype: &::arrow2::datatypes::DataType,
    includes: &mut Includes,
    is_top_level_type: bool,
) -> TokenStream {
    use arrow2::datatypes::UnionMode;
    match datatype {
        DataType::Null => quote!(arrow::null()),
        DataType::Boolean => quote!(arrow::boolean()),
        DataType::Int8 => quote!(arrow::int8()),
        DataType::Int16 => quote!(arrow::int16()),
        DataType::Int32 => quote!(arrow::int32()),
        DataType::Int64 => quote!(arrow::int64()),
        DataType::UInt8 => quote!(arrow::uint8()),
        DataType::UInt16 => quote!(arrow::uint16()),
        DataType::UInt32 => quote!(arrow::uint32()),
        DataType::UInt64 => quote!(arrow::uint64()),
        DataType::Float16 => quote!(arrow::float16()),
        DataType::Float32 => quote!(arrow::float32()),
        DataType::Float64 => quote!(arrow::float64()),
        DataType::Binary => quote!(arrow::binary()),
        DataType::LargeBinary => quote!(arrow::large_binary()),
        DataType::Utf8 => quote!(arrow::utf8()),
        DataType::LargeUtf8 => quote!(arrow::large_utf8()),

        DataType::List(field) => {
            let quoted_field = quote_arrow_field(field, includes);
            quote!(arrow::list(#quoted_field))
        }

        DataType::FixedSizeList(field, length) => {
            let quoted_field = quote_arrow_field(field, includes);
            let quoted_length = quote_integer(length);
            quote!(arrow::fixed_size_list(#quoted_field, #quoted_length))
        }

        DataType::Union(fields, _, mode) => {
            let quoted_fields = fields
                .iter()
                .map(|field| quote_arrow_field(field, includes));
            match mode {
                UnionMode::Dense => {
                    quote! { arrow::dense_union({ #(#quoted_fields,)* }) }
                }
                UnionMode::Sparse => {
                    quote! { arrow::sparse_union({ #(#quoted_fields,)* }) }
                }
            }
        }

        DataType::Struct(fields) => {
            let fields = fields
                .iter()
                .map(|field| quote_arrow_field(field, includes));
            quote! { arrow::struct_({ #(#fields,)* }) }
        }

        DataType::Extension(fqname, datatype, _metadata) => {
            // If we're not at the top level, we should have already a `to_arrow_datatype` method that we can relay to.
            if is_top_level_type {
                // TODO(andreas): We're no`t emitting the actual extension types here yet which is why we're skipping the extension type at top level.
                // Currently, we wrap only Components in extension types but this is done in `rerun_c`.
                // In the future we'll add the extension type here to the schema.
                quote_arrow_data_type(datatype, includes, false)
            } else {
                // TODO(andreas): remove unnecessary namespacing.
                let quoted_fqname = quote_fqname_as_type_path(includes, fqname);
                quote! { #quoted_fqname::to_arrow_datatype() }
            }
        }

        _ => unimplemented!("{:#?}", datatype),
    }
}

fn quote_arrow_field(field: &::arrow2::datatypes::Field, includes: &mut Includes) -> TokenStream {
    let arrow2::datatypes::Field {
        name,
        data_type,
        is_nullable,
        metadata,
    } = field;

    let datatype = quote_arrow_data_type(data_type, includes, false);

    let metadata = if metadata.is_empty() {
        quote!(nullptr)
    } else {
        let keys = metadata.keys();
        let values = metadata.values();
        quote! {
            arrow::KeyValueMetadata::Make({ #(#keys,)* }, { #(#values,)* })
        }
    };

    quote! {
        arrow::field(#name, #datatype, #is_nullable, #metadata)
    }
}
