//! Document a datatype as human-readable markdown.

use crate::codegen::StringExt as _;
use crate::data_type::{AtomicDataType, DataType, UnionMode};

fn atomic_datatype_docs(page: &mut String, datatype: &AtomicDataType) {
    match datatype {
        AtomicDataType::Null => page.push_str("null"),
        AtomicDataType::Boolean => page.push_str("boolean"),
        AtomicDataType::Int8 => page.push_str("int8"),
        AtomicDataType::Int16 => page.push_str("int16"),
        AtomicDataType::Int32 => page.push_str("int32"),
        AtomicDataType::Int64 => page.push_str("int64"),
        AtomicDataType::UInt8 => page.push_str("uint8"),
        AtomicDataType::UInt16 => page.push_str("uint16"),
        AtomicDataType::UInt32 => page.push_str("uint32"),
        AtomicDataType::UInt64 => page.push_str("uint64"),
        AtomicDataType::Float16 => page.push_str("float16"),
        AtomicDataType::Float32 => page.push_str("float32"),
        AtomicDataType::Float64 => page.push_str("float64"),
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
        DataType::Utf8 => page.push_str("utf8"),
        DataType::Binary => page.push_str("binary"),
        DataType::List(inner) => {
            page.push_str("List<");
            datatype_docs_impl(page, indent + 1, inner.data_type());
            page.push('>');
        }
        DataType::FixedSizeList(inner, length) => {
            page.push_str(&format!("FixedSizeList<{length}, "));
            datatype_docs_impl(page, indent + 1, inner.data_type());
            page.push('>');
        }
        DataType::Struct(fields) => {
            page.push_str("Struct {\n");
            for field in fields {
                page.push_indented(indent + 1, field.name(), 0);
                page.push_str(": ");
                if field.is_nullable() {
                    page.push_str("nullable ");
                }
                datatype_docs_impl(page, indent + 1, field.data_type());
                page.push('\n');
            }
            page.push_indented(indent, "}", 0);
        }
        DataType::Union(union_fields, union_mode) => {
            match union_mode {
                UnionMode::Sparse => page.push_str("SparseUnion {\n"),
                UnionMode::Dense => page.push_str("DenseUnion {\n"),
            }
            for (index, field) in union_fields.iter().enumerate() {
                page.push_indented(indent + 1, format!("{index} = {:?}: ", field.name()), 0);
                if field.is_nullable() {
                    page.push_str("nullable ");
                }
                datatype_docs_impl(page, indent + 1, field.data_type());
                page.push('\n');
            }
            page.push_indented(indent, "}", 0);
        }
        DataType::Object { datatype, .. } => {
            datatype_docs_impl(page, indent, datatype);
        }
    }
}
