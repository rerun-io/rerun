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

use arrow::array::{Array, AsArray as _, UnionArray};
use arrow::datatypes::{DataType, FieldRef};
use arrow::error::ArrowError;
use arrow::json::writer::{Encoder, EncoderFactory, EncoderOptions, NullableEncoder, make_encoder};

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{RecordBatch, RecordBatchOptions};
    use arrow::datatypes::{Field, Schema};
    use arrow::json::writer::{JsonArray, WriterBuilder};

    use re_sdk_types::ToArrowOpt as _;
    use re_sdk_types::datatypes::{TimeInt, TimeRangeBoundary};

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
}
