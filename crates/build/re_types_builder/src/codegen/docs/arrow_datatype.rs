//! Document an Arrow datatype as human-readable markdown.
//!
//! Note that we use the `arrow` library in this module,
//! with just a thin `arrow2` wrapper around it.

use arrow::datatypes::DataType;

use crate::codegen::StringExt as _;

pub fn arrow2_datatype_docs(page: &mut String, datatype: &arrow2::datatypes::DataType) {
    arrow_datatype_docs(page, 0, &DataType::from(datatype.clone()));
}

pub fn arrow_datatype_docs(page: &mut String, indent: usize, datatype: &DataType) {
    match datatype {
        DataType::Null => page.push_str("null"),
        DataType::Boolean => page.push_str("boolean"),
        DataType::Int8 => page.push_str("int8"),
        DataType::Int16 => page.push_str("int16"),
        DataType::Int32 => page.push_str("int32"),
        DataType::Int64 => page.push_str("int64"),
        DataType::UInt8 => page.push_str("uint8"),
        DataType::UInt16 => page.push_str("uint16"),
        DataType::UInt32 => page.push_str("uint32"),
        DataType::UInt64 => page.push_str("uint64"),
        DataType::Float16 => page.push_str("float16"),
        DataType::Float32 => page.push_str("float32"),
        DataType::Float64 => page.push_str("float64"),
        DataType::Utf8 => page.push_str("utf8"),
        DataType::List(inner) => {
            page.push_str("List<");
            arrow_datatype_docs(page, indent + 1, inner.data_type());
            page.push('>');
        }
        DataType::FixedSizeList(inner, length) => {
            page.push_str(&format!("FixedSizeList<{length}, "));
            arrow_datatype_docs(page, indent + 1, inner.data_type());
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
                arrow_datatype_docs(page, indent + 1, field.data_type());
                page.push('\n');
            }
            page.push_indented(indent, "}", 0);
        }
        DataType::Union(union_fields, union_mode) => {
            match union_mode {
                arrow::datatypes::UnionMode::Sparse => page.push_str("SparseUnion {\n"),
                arrow::datatypes::UnionMode::Dense => page.push_str("DenseUnion {\n"),
            }
            for (index, field) in union_fields.iter() {
                page.push_indented(indent + 1, format!("{index} = {:?}: ", field.name()), 0);
                if field.is_nullable() {
                    page.push_str("nullable ");
                }
                arrow_datatype_docs(page, indent + 1, field.data_type());
                page.push('\n');
            }
            page.push_indented(indent, "}", 0);
        }
        _ => {
            unimplemented!(
                "For the docs, you need to implement formatting of arrow datatype {:#?}",
                datatype
            );
        }
    }
}
