use proc_macro2::Ident;
use quote::{format_ident, quote};

use super::forward_decl::{ForwardDecl, ForwardDecls};
use crate::{Object, ObjectClass, Objects, Type};

pub fn arrow_array_builder_type(typ: &Type, objects: &Objects) -> Ident {
    arrow_array_builder_type_and_declaration(typ, objects, &mut ForwardDecls::default())
}

fn arrow_array_builder_type_and_declaration(
    typ: &Type,
    objects: &Objects,
    declarations: &mut ForwardDecls,
) -> Ident {
    match typ {
        Type::Unit => {
            let ident = format_ident!("NullBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }

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
                Type::Float16 => "HalfFloat",
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
        Type::Binary => {
            let ident = format_ident!("LargeBinaryBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
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
        Type::Object { fqname } => {
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
    } else if let Some(enum_type) = obj.enum_integer_type() {
        arrow_array_builder_type_and_declaration(&enum_type.to_type(), objects, declarations)
    } else {
        let class_ident = match obj.class {
            ObjectClass::Struct => format_ident!("StructBuilder"),
            ObjectClass::Union => format_ident!("DenseUnionBuilder"),
            ObjectClass::Enum(_) => unreachable!(),
        };

        declarations.insert("arrow", ForwardDecl::Class(class_ident.clone()));
        class_ident
    }
}
