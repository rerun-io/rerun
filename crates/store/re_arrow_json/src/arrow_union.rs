//! Arrow unions as JSON, which `arrow-json` cannot encode on its own.
//!
//! Each value is written externally tagged, by the name of its variant:
//! * a variant carrying data as `{ "Variant": data }`, its data encoded as any other value;
//! * a variant with no data (a `Null` child) as `"Variant"`;
//! * Rerun's `_null_markers` variant as `null`.
//!
//! For example, `TimeRangeBoundary` is a Rust enum stored as a dense union, with type id 0 kept
//! for `_null_markers`:
//!
//! ```ignore
//! enum TimeRangeBoundary {
//!     CursorRelative(TimeInt), // type id 1
//!     Absolute(TimeInt),       // type id 2
//!     Infinite,                // type id 3, a `Null` child
//! }
//!
//! struct TimeRange {
//!     start: TimeRangeBoundary,
//!     end: TimeRangeBoundary,
//! }
//! ```
//!
//! `TimeRange { start: CursorRelative(TimeInt(-100)), end: Infinite }` is then written as:
//!
//! ```json
//! { "start": { "CursorRelative": -100 }, "end": "Infinite" }
//! ```
//!
//! A null boundary is written as `null`.
//!
//! `arrow-json` cannot decode a union either, and offers no hook to, so [`array_from_json`] builds
//! any datatype holding a union itself, and hands the parts without one back to `arrow-json`.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, AsArray as _, FixedSizeListArray, ListArray, NullArray, StructArray,
    UnionArray,
};
use arrow::buffer::{NullBuffer, OffsetBuffer};
use arrow::datatypes::{DataType, FieldRef, Fields, UnionFields, UnionMode};
use arrow::error::ArrowError;
use arrow::json::writer::{Encoder, EncoderFactory, EncoderOptions, NullableEncoder, make_encoder};
use serde_json::Value;

use crate::from_json::{ValueFromJsonError, arrow_from_json};

/// The variant Rerun's codegen puts at type id 0 of every union, to encode a null value.
const NULL_MARKERS: &str = "_null_markers";

/// Makes `arrow-json` encode unions, at any depth, as the module docs describe.
#[derive(Debug)]
pub struct UnionEncoderFactory;

impl EncoderFactory for UnionEncoderFactory {
    fn make_default_encoder<'a>(
        &self,
        _field: &'a FieldRef,
        array: &'a dyn Array,
        options: &'a EncoderOptions,
    ) -> Result<Option<NullableEncoder<'a>>, ArrowError> {
        let DataType::Union(fields, _) = array.data_type() else {
            return Ok(None);
        };
        let union = array.as_union();

        let mut variants = Vec::new();
        for (type_id, field) in fields.iter() {
            let encoder = if field.data_type() == &DataType::Null {
                None
            } else {
                Some(make_encoder(field, union.child(type_id).as_ref(), options)?)
            };
            variants.push(Variant {
                type_id,
                name: serde_json::to_string(field.name())
                    .map_err(|err| ArrowError::JsonError(err.to_string()))?,
                is_null_marker: field.name() == NULL_MARKERS,
                encoder,
            });
        }

        Ok(Some(NullableEncoder::new(
            Box::new(UnionEncoder { union, variants }),
            None,
        )))
    }
}

struct Variant<'a> {
    type_id: i8,

    /// The variant's name, already quoted and escaped as a JSON string.
    name: String,

    is_null_marker: bool,

    /// `None` for a variant with no data.
    encoder: Option<NullableEncoder<'a>>,
}

struct UnionEncoder<'a> {
    union: &'a UnionArray,
    variants: Vec<Variant<'a>>,
}

impl Encoder for UnionEncoder<'_> {
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        let type_id = self.union.type_id(idx);
        let offset = self.union.value_offset(idx);
        let Some(variant) = self
            .variants
            .iter_mut()
            .find(|variant| variant.type_id == type_id)
        else {
            // A type id the datatype does not list: the array is malformed.
            out.extend_from_slice(b"null");
            return;
        };

        if variant.is_null_marker {
            out.extend_from_slice(b"null");
            return;
        }
        let Some(encoder) = &mut variant.encoder else {
            out.extend_from_slice(variant.name.as_bytes());
            return;
        };
        out.push(b'{');
        out.extend_from_slice(variant.name.as_bytes());
        out.push(b':');
        if encoder.is_null(offset) {
            out.extend_from_slice(b"null");
        } else {
            encoder.encode(offset, out);
        }
        out.push(b'}');
    }
}

/// Whether `datatype` holds a union anywhere, which only [`array_from_json`] can decode.
pub fn contains_union(datatype: &DataType) -> bool {
    match datatype {
        DataType::Union(..) => true,
        DataType::Struct(fields) => fields.iter().any(|field| contains_union(field.data_type())),
        DataType::List(field) | DataType::LargeList(field) | DataType::FixedSizeList(field, _) => {
            contains_union(field.data_type())
        }
        _ => false,
    }
}

/// `values` as an array of `datatype`, which holds a union somewhere, in the form the module docs
/// describe.
pub fn array_from_json(
    datatype: &DataType,
    values: &[Value],
) -> Result<ArrayRef, ValueFromJsonError> {
    // The recursion below only handles what can hold a union; everything else, including the
    // union-free parts of a type that does hold one, is decoded by `arrow-json`.
    if contains_union(datatype) {
        match datatype {
            DataType::Union(fields, UnionMode::Dense) => union_from_json(fields, values),
            DataType::Struct(fields) => struct_from_json(fields, values),
            DataType::List(field) => list_from_json(field, None, values),
            DataType::FixedSizeList(field, size) => list_from_json(field, Some(*size), values),
            _ => Err(ValueFromJsonError::UnsupportedDatatype {
                datatype: datatype.clone(),
            }),
        }
    } else {
        arrow_from_json(datatype, values)
    }
}

fn union_from_json(fields: &UnionFields, values: &[Value]) -> Result<ArrayRef, ValueFromJsonError> {
    const EXPECTED: &str = r#"a variant: "Variant", { "Variant": value }, or null"#;

    let variant_names = || {
        fields
            .iter()
            .map(|(_, field)| field.name().as_str())
            .filter(|name| *name != NULL_MARKERS)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let type_id_of = |name: &str| fields.iter().find(|(_, field)| field.name() == name);
    let build_error = |err| ValueFromJsonError::BuildArray {
        datatype: DataType::Union(fields.clone(), UnionMode::Dense),
        err,
    };

    let mut type_ids = Vec::with_capacity(values.len());
    let mut offsets = Vec::with_capacity(values.len());
    let mut child_values: BTreeMap<i8, Vec<Value>> = BTreeMap::new();
    for value in values {
        let (name, data) = match value {
            Value::Null => (NULL_MARKERS, Value::Null),
            Value::String(name) => (name.as_str(), Value::Null),
            Value::Object(object) => match object.iter().next() {
                Some((name, data)) if object.len() == 1 => (name.as_str(), data.clone()),
                _ => return Err(ValueFromJsonError::expected(EXPECTED, value)),
            },
            _ => return Err(ValueFromJsonError::expected(EXPECTED, value)),
        };
        let Some((type_id, field)) = type_id_of(name) else {
            return Err(ValueFromJsonError::UnknownEnumVariant {
                name: name.to_owned(),
                expected: variant_names(),
            });
        };
        // A variant without data is written as its name alone, and one with data as an object.
        let has_data = field.data_type() != &DataType::Null;
        if has_data != value.is_object() {
            return Err(ValueFromJsonError::expected(EXPECTED, value));
        }

        let child = child_values.entry(type_id).or_default();
        type_ids.push(type_id);
        offsets.push(
            i32::try_from(child.len())
                .map_err(|err| ArrowError::InvalidArgumentError(format!("too many values: {err}")))
                .map_err(build_error)?,
        );
        child.push(data);
    }

    let children = fields
        .iter()
        .map(|(type_id, field)| {
            let values = child_values.remove(&type_id).unwrap_or_default();
            if field.data_type() == &DataType::Null {
                Ok(Arc::new(NullArray::new(values.len())) as ArrayRef)
            } else {
                array_from_json(field.data_type(), &values)
            }
        })
        .collect::<Result<Vec<_>, ValueFromJsonError>>()?;

    Ok(Arc::new(
        UnionArray::try_new(
            fields.clone(),
            type_ids.into(),
            Some(offsets.into()),
            children,
        )
        .map_err(build_error)?,
    ))
}

fn struct_from_json(fields: &Fields, values: &[Value]) -> Result<ArrayRef, ValueFromJsonError> {
    for value in values {
        match value {
            Value::Null => {}
            Value::Object(object) => {
                // As strict as `arrow-json` is on structs without a union.
                if let Some(unknown) = object.keys().find(|key| fields.find(key).is_none()) {
                    return Err(ValueFromJsonError::UnknownField {
                        name: unknown.clone(),
                        expected: fields
                            .iter()
                            .map(|field| field.name().as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    });
                }
            }
            _ => return Err(ValueFromJsonError::expected("an object", value)),
        }
    }
    let children = fields
        .iter()
        .map(|field| {
            let column: Vec<Value> = values
                .iter()
                .map(|value| value.get(field.name()).cloned().unwrap_or(Value::Null))
                .collect();
            array_from_json(field.data_type(), &column)
        })
        .collect::<Result<Vec<_>, ValueFromJsonError>>()?;
    Ok(Arc::new(
        StructArray::try_new(fields.clone(), children, nulls_of(values)).map_err(|err| {
            ValueFromJsonError::BuildArray {
                datatype: DataType::Struct(fields.clone()),
                err,
            }
        })?,
    ))
}

/// A list array of `field`, fixed to `size` elements per list if given.
fn list_from_json(
    field: &FieldRef,
    size: Option<i32>,
    values: &[Value],
) -> Result<ArrayRef, ValueFromJsonError> {
    let lists = FlattenedLists::from_json(values)?;
    if let Some(size) = size
        && let Some((_, value)) = std::iter::zip(&lists.lengths, values)
            .find(|(length, value)| !value.is_null() && i32::try_from(**length).ok() != Some(size))
    {
        return Err(ValueFromJsonError::expected(
            format!("an array of {size} elements"),
            value,
        ));
    }

    let elements = array_from_json(field.data_type(), &lists.elements)?;
    let nulls = nulls_of(values);
    let built: Result<ArrayRef, ArrowError> = match size {
        Some(size) => FixedSizeListArray::try_new(field.clone(), size, elements, nulls)
            .map(|array| Arc::new(array) as ArrayRef),
        None => ListArray::try_new(
            field.clone(),
            OffsetBuffer::from_lengths(lists.lengths),
            elements,
            nulls,
        )
        .map(|array| Arc::new(array) as ArrayRef),
    };
    built.map_err(|err| ValueFromJsonError::BuildArray {
        datatype: match size {
            Some(size) => DataType::FixedSizeList(field.clone(), size),
            None => DataType::List(field.clone()),
        },
        err,
    })
}

/// A column of JSON arrays, flattened the way Arrow stores a list column.
struct FlattenedLists {
    /// How many elements each list has. A null list has none.
    lengths: Vec<usize>,

    /// Every list's elements, in order.
    elements: Vec<Value>,
}

impl FlattenedLists {
    fn from_json(values: &[Value]) -> Result<Self, ValueFromJsonError> {
        let mut lengths = Vec::with_capacity(values.len());
        let mut elements = Vec::new();
        for value in values {
            match value {
                Value::Array(list) => {
                    lengths.push(list.len());
                    elements.extend(list.iter().cloned());
                }
                Value::Null => lengths.push(0),
                _ => return Err(ValueFromJsonError::expected("an array", value)),
            }
        }
        Ok(Self { lengths, elements })
    }
}

/// Which of `values` are null, if any are.
fn nulls_of(values: &[Value]) -> Option<NullBuffer> {
    values
        .iter()
        .any(Value::is_null)
        .then(|| values.iter().map(|value| !value.is_null()).collect())
}

#[cfg(test)]
mod tests {
    use arrow::array::{RecordBatch, RecordBatchOptions};
    use arrow::datatypes::{Field, Schema};
    use arrow::json::writer::{JsonArray, WriterBuilder};

    use re_sdk_types::datatypes::{TimeInt, TimeRangeBoundary};
    use re_sdk_types::{FromArrowOpt as _, ToArrowOpt as _};

    use super::*;

    #[test]
    fn every_kind_of_variant() {
        let boundaries = TimeRangeBoundary::to_arrow_opt([
            Some(TimeRangeBoundary::CursorRelative(TimeInt(-100))),
            Some(TimeRangeBoundary::Infinite),
            None,
        ])
        .unwrap();
        let schema = Schema::new_with_metadata(
            vec![Field::new("b", boundaries.data_type().clone(), true)],
            Default::default(),
        );
        let options = RecordBatchOptions::new().with_row_count(Some(boundaries.len()));
        let batch = RecordBatch::try_new_with_options(Arc::new(schema), vec![boundaries], &options)
            .unwrap();

        let mut writer = WriterBuilder::new()
            .with_explicit_nulls(true)
            .with_encoder_factory(Arc::new(UnionEncoderFactory))
            .build::<_, JsonArray>(Vec::new());
        writer.write(&batch).unwrap();
        writer.finish().unwrap();

        assert_eq!(
            String::from_utf8(writer.into_inner()).unwrap(),
            r#"[{"b":{"CursorRelative":-100}},{"b":"Infinite"},{"b":null}]"#
        );
    }

    #[test]
    fn every_kind_of_variant_is_decoded() {
        let expected = TimeRangeBoundary::to_arrow_opt([
            Some(TimeRangeBoundary::CursorRelative(TimeInt(-100))),
            Some(TimeRangeBoundary::Infinite),
            None,
        ])
        .unwrap();

        let decoded = array_from_json(
            expected.data_type(),
            &[
                serde_json::json!({ "CursorRelative": -100 }),
                serde_json::json!("Infinite"),
                Value::Null,
            ],
        )
        .unwrap();

        assert_eq!(
            TimeRangeBoundary::from_arrow_opt(decoded.as_ref()).unwrap(),
            vec![
                Some(TimeRangeBoundary::CursorRelative(TimeInt(-100))),
                Some(TimeRangeBoundary::Infinite),
                None,
            ]
        );
    }
}
