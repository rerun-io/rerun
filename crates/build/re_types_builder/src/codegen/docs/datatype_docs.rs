//! Document a datatype as human-readable markdown.

use crate::codegen::StringExt as _;
use crate::data_type::{AtomicDataType, DataType, UnionMode};

fn atomic_datatype_docs(page: &mut String, datatype: &AtomicDataType) {
    match datatype {
        AtomicDataType::Null => page.push_str("Null"),
        AtomicDataType::Boolean => page.push_str("Boolean"),
        AtomicDataType::Int8 => page.push_str("Int8"),
        AtomicDataType::Int16 => page.push_str("Int16"),
        AtomicDataType::Int32 => page.push_str("Int32"),
        AtomicDataType::Int64 => page.push_str("Int64"),
        AtomicDataType::UInt8 => page.push_str("UInt8"),
        AtomicDataType::UInt16 => page.push_str("UInt16"),
        AtomicDataType::UInt32 => page.push_str("UInt32"),
        AtomicDataType::UInt64 => page.push_str("UInt64"),
        AtomicDataType::Float16 => page.push_str("Float16"),
        AtomicDataType::Float32 => page.push_str("Float32"),
        AtomicDataType::Float64 => page.push_str("Float64"),
    }
}

pub fn datatype_docs(page: &mut String, datatype: &DataType) {
    datatype_docs_impl(page, 0, datatype);
}

fn datatype_docs_impl(page: &mut String, indent: usize, datatype: &DataType) {
    match datatype {
        DataType::Atomic(atomic) => {
            atomic_datatype_docs(page, atomic);
        }
        DataType::Utf8 => page.push_str("Utf8"),
        DataType::Binary => page.push_str("Binary"),
        DataType::List(inner) => {
            page.push_str("List(");
            if !inner.is_nullable() {
                // This follows the notation set by arrow-rs.
                // If we change this, we should probably change
                // arrow-rs and datafusion to match.
                page.push_str("non-null ");
            }
            datatype_docs_impl(page, indent + 1, inner.data_type());
            page.push(')');
        }
        DataType::FixedSizeList(inner, length) => {
            page.push_str(&format!("FixedSizeList({length} x "));
            if !inner.is_nullable() {
                page.push_str("non-null ");
            }
            datatype_docs_impl(page, indent + 1, inner.data_type());
            page.push(')');
        }
        DataType::Struct(fields) => {
            page.push_str("Struct(\n");
            for field in fields {
                page.push_indented(indent + 1, format!("{:?}: ", field.name()), 0);
                if !field.is_nullable() {
                    page.push_str("non-null ");
                }
                datatype_docs_impl(page, indent + 1, field.data_type());
                page.push('\n');
            }
            page.push_indented(indent, ")", 0);
        }
        DataType::Union(union_fields, union_mode) => {
            match union_mode {
                UnionMode::Sparse => page.push_str("Union(Sparse,\n"),
                UnionMode::Dense => page.push_str("Union(Dense,\n"),
            }
            for (index, field) in union_fields.iter().enumerate() {
                page.push_indented(indent + 1, format!("{index}: ({:?}: ", field.name()), 0);
                if !field.is_nullable() {
                    page.push_str("non-null ");
                }
                datatype_docs_impl(page, indent + 1, field.data_type());
                page.push_str(")\n");
            }
            page.push_indented(indent, ")", 0);
        }
        DataType::Object { datatype, .. } => {
            datatype_docs_impl(page, indent, datatype);
        }
    }
}
