//! Pins the Arrow type-ids of every union reachable from a component.
//!
//! Type-ids are wire format: they are written into every serialized union array, and old
//! recordings decode against whatever mapping was in effect when they were written.
//! Nothing in the codegen pipeline states the mapping explicitly — it falls out of the order the
//! variants happen to appear in — so reordering variants in a type definition silently
//! reinterprets old data as the wrong variant.
//!
//! This test exists so that such a change shows up as a small, unmissable diff rather than being
//! buried in the regenerated serializer.
//!
//! If it fails and you *did* mean to change the wire format, accept the new snapshot and say so
//! in the changelog. If you did not, put the variants back in their original order and append new
//! ones at the end.
//!
//! We want to get rid of Arrow unions entirely — see
//! <https://github.com/rerun-io/rerun/issues/6388> — at which point this test has nothing left to
//! guard and can go away.

use std::collections::BTreeSet;

use arrow::datatypes::DataType;

/// Snapshots every union reachable from a component, one block per union, sorted.
#[test]
fn union_type_ids_are_stable() {
    let reflection = re_sdk_types::reflection::generate_reflection()
        .expect("failed to generate component reflection");

    let mut unions = BTreeSet::new();
    #[expect(clippy::iter_over_hash_type)] // the results land in a `BTreeSet`, so order is moot
    for reflection in reflection.components.values() {
        collect_unions(&reflection.datatype, &mut unions);
    }

    insta::assert_snapshot!(unions.into_iter().collect::<Vec<_>>().join("\n"), @r"
    0 = _null_markers: Null
    1 = CursorRelative: Int64
    2 = Absolute: Int64
    3 = Infinite: Null

    0 = _null_markers: Null
    1 = U8: List
    2 = U16: List
    3 = U32: List
    4 = U64: List
    5 = I8: List
    6 = I16: List
    7 = I32: List
    8 = I64: List
    9 = F16: List
    10 = F32: List
    11 = F64: List
    ");
}

/// Recursively collects every [`DataType::Union`] in `datatype`, formatted as a block of
/// `type_id = name: type` lines.
///
/// Unions are keyed by that rendering rather than by name, because an Arrow datatype carries no
/// name — the same union reached through two different components dedupes into one entry.
fn collect_unions(datatype: &DataType, found: &mut BTreeSet<String>) {
    match datatype {
        DataType::Union(fields, _mode) => {
            let block = fields
                .iter()
                .map(|(type_id, field)| {
                    format!(
                        "{type_id} = {}: {}\n",
                        field.name(),
                        shape(field.data_type())
                    )
                })
                .collect::<String>();

            // `insert` returns false if we have already walked this union, which also means we
            // have already walked its variants — so only recurse the first time.
            if found.insert(block) {
                for (_type_id, field) in fields.iter() {
                    collect_unions(field.data_type(), found);
                }
            }
        }

        DataType::Struct(fields) => {
            for field in fields {
                collect_unions(field.data_type(), found);
            }
        }

        DataType::List(field)
        | DataType::LargeList(field)
        | DataType::FixedSizeList(field, _)
        | DataType::ListView(field)
        | DataType::LargeListView(field) => collect_unions(field.data_type(), found),

        // Leaves: nothing to recurse into.
        DataType::Null
        | DataType::Boolean
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
        | DataType::Timestamp(_, _)
        | DataType::Date32
        | DataType::Date64
        | DataType::Time32(_)
        | DataType::Time64(_)
        | DataType::Duration(_)
        | DataType::Interval(_)
        | DataType::Binary
        | DataType::FixedSizeBinary(_)
        | DataType::LargeBinary
        | DataType::BinaryView
        | DataType::Utf8
        | DataType::LargeUtf8
        | DataType::Utf8View
        | DataType::Decimal32(_, _)
        | DataType::Decimal64(_, _)
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _) => {}

        // We don't generate these, and would need to think about how they nest before walking them.
        DataType::Dictionary(_, _) | DataType::Map(_, _) | DataType::RunEndEncoded(_, _) => {
            panic!("Unexpected datatype that might contain unions: {datatype:?}")
        }
    }
}

/// The outer shape of a datatype, without its contents.
///
/// The contents are irrelevant here — this test is about which variant a type-id names, not about
/// what that variant holds — and leaving them out keeps the snapshot readable.
fn shape(datatype: &DataType) -> &'static str {
    match datatype {
        DataType::Null => "Null",
        DataType::Boolean => "Boolean",
        DataType::Int8 => "Int8",
        DataType::Int16 => "Int16",
        DataType::Int32 => "Int32",
        DataType::Int64 => "Int64",
        DataType::UInt8 => "UInt8",
        DataType::UInt16 => "UInt16",
        DataType::UInt32 => "UInt32",
        DataType::UInt64 => "UInt64",
        DataType::Float16 => "Float16",
        DataType::Float32 => "Float32",
        DataType::Float64 => "Float64",
        DataType::Binary => "Binary",
        DataType::Utf8 => "Utf8",
        DataType::List(_) => "List",
        DataType::FixedSizeList(..) => "FixedSizeList",
        DataType::Struct(_) => "Struct",
        DataType::Union(..) => "Union",
        other => panic!("Unexpected datatype in a union variant: {other:?}"),
    }
}
