use proc_macro2::Ident;
use quote::{format_ident, quote};

use super::forward_decl::{ForwardDecl, ForwardDecls};
use crate::{AtomicDataType, Object, ObjectClass, Objects, Type};

pub fn arrow_array_builder_type(typ: &Type, objects: &Objects) -> Ident {
    arrow_array_builder_type_and_declaration(typ, objects, &mut ForwardDecls::default())
}

/// What arrow's C++ classes for `atomic` are called, e.g. `arrow::HalfFloatBuilder` and
/// `arrow::HalfFloatType` for a `f16`.
pub fn arrow_class_prefix(atomic: AtomicDataType) -> &'static str {
    match atomic {
        AtomicDataType::Null => "Null",
        AtomicDataType::Boolean => "Boolean",
        AtomicDataType::Int8 => "Int8",
        AtomicDataType::Int16 => "Int16",
        AtomicDataType::Int32 => "Int32",
        AtomicDataType::Int64 => "Int64",
        AtomicDataType::UInt8 => "UInt8",
        AtomicDataType::UInt16 => "UInt16",
        AtomicDataType::UInt32 => "UInt32",
        AtomicDataType::UInt64 => "UInt64",
        AtomicDataType::Float16 => "HalfFloat",
        AtomicDataType::Float32 => "Float",
        AtomicDataType::Float64 => "Double",
    }
}

pub fn arrow_builder_ident(atomic: AtomicDataType) -> Ident {
    format_ident!("{}Builder", arrow_class_prefix(atomic))
}

fn arrow_array_builder_type_and_declaration(
    typ: &Type,
    objects: &Objects,
    declarations: &mut ForwardDecls,
) -> Ident {
    match typ {
        // The numeric builders are all `arrow::NumericBuilder<T>` aliases, the other two are
        // classes of their own.
        Type::Atomic(atomic @ (AtomicDataType::Null | AtomicDataType::Boolean)) => {
            let ident = arrow_builder_ident(*atomic);
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }
        Type::Atomic(atomic) => {
            let klass_type = format_ident!("{}Type", arrow_class_prefix(*atomic));

            declarations.insert(
                "arrow",
                ForwardDecl::TemplateClass(format_ident!("NumericBuilder")),
            );
            declarations.insert("arrow", ForwardDecl::Class(klass_type.clone()));

            let ident = arrow_builder_ident(*atomic);
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
        Type::Utf8 => {
            let ident = format_ident!("StringBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }
        Type::FixedSizeList { .. } => {
            let ident = format_ident!("FixedSizeListBuilder");
            declarations.insert("arrow", ForwardDecl::Class(ident.clone()));
            ident
        }
        Type::List { .. } => {
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
