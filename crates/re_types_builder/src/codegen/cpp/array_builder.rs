use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::{Object, ObjectSpecifics, Objects, Type};

use super::{
    forward_decl::{ForwardDecl, ForwardDecls},
    includes::Includes,
    quote_fqname_as_type_path, quote_integer,
};

pub fn arrow_array_builder_type(typ: &Type, objects: &Objects) -> Ident {
    arrow_array_builder_type_and_declaration(typ, objects, &mut ForwardDecls::default())
}

fn arrow_array_builder_type_and_declaration(
    typ: &Type,
    objects: &Objects,
    declarations: &mut ForwardDecls,
) -> Ident {
    match typ {
        Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Float16
        | Type::Float32
        | Type::Float64 => {
            let klass = match typ {
                Type::Int8 => "Int8",
                Type::Int16 => "Int16",
                Type::Int32 => "Int32",
                Type::Int64 => "Int64",
                Type::UInt8 => "UInt8",
                Type::UInt16 => "UInt16",
                Type::UInt32 => "UInt32",
                Type::UInt64 => "UInt64",
                Type::Float16 => "Float16",
                Type::Float32 => "Float",
                Type::Float64 => "Double",
                _ => {
                    unreachable!();
                }
            };
            let klass_type = format_ident!("{klass}Type");

            declarations.insert(
                "arrow",
                ForwardDecl::TemplateClass(format_ident!("NumericBuilder")),
            );
            declarations.insert("arrow", ForwardDecl::Class(klass_type.clone()));

            let ident = format_ident!("{klass}Builder");
            declarations.insert(
                "arrow",
                ForwardDecl::Alias {
                    from: ident.clone(),
                    to: quote!(NumericBuilder<#klass_type>),
                },
            );
            ident
        }
        Type::String => {
            let ident = format_ident!("StringBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }
        Type::Bool => {
            let ident = format_ident!("BooleanBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }
        Type::Array { .. } => {
            let ident = format_ident!("FixedSizeListBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }
        Type::Vector { .. } => {
            let ident = format_ident!("ListBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }
        Type::Object(fqname) => {
            arrow_array_builder_type_object(&objects[fqname], objects, declarations)
        }
    }
}

pub fn arrow_array_builder_type_object(
    obj: &Object,
    objects: &Objects,
    declarations: &mut ForwardDecls,
) -> Ident {
    if obj.is_arrow_transparent() {
        arrow_array_builder_type_and_declaration(&obj.fields[0].typ, objects, declarations)
    } else {
        let class_ident = match obj.specifics {
            ObjectSpecifics::Struct => format_ident!("StructBuilder"),
            ObjectSpecifics::Union { .. } => format_ident!("DenseUnionBuilder"),
        };

        declarations.insert("arrow", ForwardDecl::Class(class_ident.clone()));
        class_ident
    }
}

pub fn quote_arrow_array_builder_type_instantiation(
    typ: &Type,
    objects: &Objects,
    cpp_includes: &mut Includes,
    is_top_level_type: bool,
) -> TokenStream {
    let builder_type = arrow_array_builder_type(typ, objects);

    match typ {
        Type::UInt8
        | Type::UInt16
        | Type::UInt32
        | Type::UInt64
        | Type::Int8
        | Type::Int16
        | Type::Int32
        | Type::Int64
        | Type::Bool
        | Type::Float16
        | Type::Float32
        | Type::Float64
        | Type::String => {
            quote!(std::make_shared<arrow::#builder_type>(memory_pool))
        }
        Type::Vector { elem_type } => {
            let element_builder = quote_arrow_array_builder_type_instantiation(
                &elem_type.clone().into(),
                objects,
                cpp_includes,
                false,
            );
            quote!(std::make_shared<arrow::#builder_type>(memory_pool, #element_builder))
        }
        Type::Array { elem_type, length } => {
            let quoted_length = quote_integer(length);
            let element_builder = quote_arrow_array_builder_type_instantiation(
                &elem_type.clone().into(),
                objects,
                cpp_includes,
                false,
            );
            quote!(std::make_shared<arrow::#builder_type>(memory_pool, #element_builder, #quoted_length))
        }
        Type::Object(fqname) => {
            let object = &objects[fqname];

            if !is_top_level_type {
                // Propagating error here is hard since we're in a nested context.
                // But also not that important since we *know* that this only fails for null pools and we already checked that now.
                // For the unlikely broken case, Rerun result will give us a nullptr which will then
                // fail the subsequent actions inside arrow, so the error will still propagate.
                let quoted_fqname = quote_fqname_as_type_path(cpp_includes, fqname);
                quote!(#quoted_fqname::new_arrow_array_builder(memory_pool).value)
            } else if object.is_arrow_transparent() {
                quote_arrow_array_builder_type_instantiation(
                    &object.fields[0].typ,
                    objects,
                    cpp_includes,
                    false,
                )
            } else {
                let field_builders = object.fields.iter().map(|field| {
                    quote_arrow_array_builder_type_instantiation(
                        &field.typ,
                        objects,
                        cpp_includes,
                        false,
                    )
                });

                match object.specifics {
                    ObjectSpecifics::Struct => {
                        quote! {
                            std::make_shared<arrow::#builder_type>(
                                arrow_datatype(),
                                memory_pool,
                                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({ #(#field_builders,)* })
                            )
                        }
                    }
                    ObjectSpecifics::Union { .. } => {
                        quote! {
                             std::make_shared<arrow::#builder_type>(
                                 memory_pool,
                                 std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                                     std::make_shared<arrow::NullBuilder>(memory_pool), #(#field_builders,)*
                                 }),
                                 arrow_datatype()
                             )
                        }
                    }
                }
            }
        }
    }
}
