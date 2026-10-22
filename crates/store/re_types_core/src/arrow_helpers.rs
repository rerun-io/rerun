use std::collections::HashMap;

use arrow::array::{Array, ArrayRef, StringArray, StructArray};
use arrow::buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow::datatypes::DataType;

use crate::{
    ArrowString, DeserializationError, DeserializationResult, ResultExt as _, SerializationResult,
};

/// A single buffer is shared without copying its bytes; multiple buffers are concatenated.
pub fn concat_u8_buffers(buffers: &mut dyn Iterator<Item = &ScalarBuffer<u8>>) -> ScalarBuffer<u8> {
    match (buffers.next(), buffers.next()) {
        // Single buffer: cheap Arc clone, avoiding allocation.
        (Some(single), None) => single.clone(),

        // Multiple buffers: single Vec allocation for slices before concatenation.
        (Some(first), Some(second)) => std::iter::chain(
            [first.as_ref(), second.as_ref()],
            buffers.map(|buffer| buffer.as_ref()),
        )
        .collect::<Vec<_>>()
        .concat()
        .into(),
        _ => ScalarBuffer::from(Vec::new()),
    }
}

/// Downcasting alone cannot distinguish fixed-size-list widths.
/// Returns the child array only after checking the expected width.
pub fn fixed_size_list_values<'a>(
    array: &'a dyn Array,
    width: i32,
    expected: fn() -> DataType,
    context: &'static str,
) -> DeserializationResult<&'a dyn Array> {
    let array = array
        .try_cast::<arrow::array::FixedSizeListArray>(expected)
        .with_context(context)?;

    // Downcasting to `FixedSizeListArray` succeeds for _any_ list width,
    // so check the width explicitly rather than let a subsequent cast trip over it.
    if array.value_length() != width {
        return Err(DeserializationError::datatype_mismatch(
            expected(),
            array.data_type().clone(),
        ))
        .with_context(context);
    }

    Ok(array.values().as_ref())
}

/// Each dense-union offset counts preceding occurrences of the same type ID.
/// Type zero is counted like any other child, including when it encodes nulls.
pub fn dense_union_offsets(type_ids: &[i8]) -> SerializationResult<ScalarBuffer<i32>> {
    let mut counts = [0_u32; 128];
    let mut offsets = Vec::with_capacity(type_ids.len());

    for &type_id in type_ids {
        let count = counts.get_mut(type_id as usize).ok_or_else(|| {
            arrow::error::ArrowError::InvalidArgumentError(format!(
                "Negative dense-union type ID: {type_id}"
            ))
        })?;
        if *count > i32::MAX as u32 {
            return Err(arrow::error::ArrowError::InvalidArgumentError(format!(
                "Dense-union child offset exceeds i32 for type ID {type_id}"
            ))
            .into());
        }
        offsets.push(count.cast_signed());
        *count += 1;
    }

    Ok(offsets.into())
}

/// Name-based access to struct children, independent of their serialized order.
/// When names repeat, the last child wins.
pub struct StructFields<'a> {
    arrays: HashMap<&'a str, &'a ArrayRef>,
    expected: fn() -> DataType,
    context: &'static str,
}

impl<'a> StructFields<'a> {
    pub fn new(array: &'a StructArray, expected: fn() -> DataType, context: &'static str) -> Self {
        Self {
            arrays: std::iter::zip(
                array.fields().iter().map(|field| field.name().as_str()),
                array.columns(),
            )
            .collect(),
            expected,
            context,
        }
    }

    pub fn required(&self, name: &'static str) -> DeserializationResult<&'a dyn Array> {
        // NOTE: `arrays` contains the runtime payload's struct fields, while `name` is
        // the field expected at compile time… there's no guarantee it's present at runtime!
        self.arrays
            .get(name)
            .map(|&array| array.as_ref())
            .ok_or_else(|| DeserializationError::missing_struct_field((self.expected)(), name))
            .with_context(self.context)
    }
}

/// Serializes UTF-8 values with an independently supplied validity bitmap.
pub fn serialize_strings<'a>(
    values: impl Iterator<Item = &'a str> + Clone,
    nulls: Option<NullBuffer>,
) -> SerializationResult<ArrayRef> {
    serialize_optional_strings(values.map(Some), nulls)
}

/// Null slots occupy no bytes; their validity is supplied by the caller.
pub fn serialize_optional_strings<'a>(
    mut values: impl Iterator<Item = Option<&'a str>> + Clone,
    nulls: Option<NullBuffer>,
) -> SerializationResult<ArrayRef> {
    let capacity = values.clone().flatten().map(str::len).sum();
    serialize_string_bytes(&mut values, capacity, nulls)
}

fn serialize_string_bytes(
    values: &mut dyn Iterator<Item = Option<&str>>,
    capacity: usize,
    nulls: Option<NullBuffer>,
) -> SerializationResult<ArrayRef> {
    let mut builder = arrow::array::builder::BufferBuilder::<u8>::new(capacity);
    let offsets = OffsetBuffer::from_lengths(values.map(|value| {
        let value = value.unwrap_or_default();
        builder.append_slice(value.as_bytes());
        value.len()
    }));
    if let Some(nulls) = &nulls
        && nulls.len() != offsets.len() - 1
    {
        return Err(arrow::error::ArrowError::InvalidArgumentError(format!(
            "String validity length {} does not match value count {}",
            nulls.len(),
            offsets.len() - 1,
        ))
        .into());
    }
    // SAFETY: Offsets are derived from the same immutable strings copied into the buffer,
    // so offsets are in bounds and on UTF-8 boundaries. Validity length is checked above.
    #[expect(unsafe_code)]
    Ok(as_array_ref(unsafe {
        StringArray::new_unchecked(offsets, builder.finish(), nulls)
    }))
}

/// Shares the input's backing buffer, including when the input array is sliced.
pub fn deserialize_strings(array: &StringArray) -> DeserializationResult<Vec<Option<ArrowString>>> {
    use crate::arrow_zip_validity::ZipValidity;

    let buffer = array.values();
    ZipValidity::new_with_validity(array.offsets().array_windows(), array.nulls())
        .map(|entry| {
            entry
                .map(|&[start, end]| {
                    // NOTE: Do _not_ use `Buffer::sliced`, it panics on malformed inputs.

                    let start = start as usize;
                    let end = end as usize;

                    // NOTE: It is absolutely crucial we explicitly handle the
                    // boundchecks manually first, otherwise rustc completely chokes
                    // when slicing the data (as in: a 100x perf drop)!
                    if buffer.len() < end {
                        return Err(DeserializationError::offset_slice_oob(
                            (start, end),
                            buffer.len(),
                        ));
                    }
                    // TODO(apache/arrow-rs#6900): slice_with_length_unchecked unsafe when https://github.com/apache/arrow-rs/pull/6901 is merged and released
                    Ok(ArrowString::from(
                        buffer.slice_with_length(start, end - start),
                    ))
                })
                .transpose()
        })
        .collect()
}

/// Omits the validity buffer when every slot is valid, including empty inputs.
pub fn validity_from_bools(somes: Vec<bool>) -> Option<NullBuffer> {
    somes.iter().any(|some| !*some).then(|| somes.into())
}

/// Repeats each parent's validity for its fixed-size-list children.
/// An absent parent bitmap stays absent; an explicit all-valid bitmap stays present.
#[expect(
    clippy::single_option_map,
    reason = "Keep the null-handling branch out of generated serializers"
)]
pub fn expand_validity(validity: Option<&NullBuffer>, width: usize) -> Option<NullBuffer> {
    validity.map(|validity| {
        validity
            .iter()
            .flat_map(|valid| std::iter::repeat_n(valid, width))
            .collect::<Vec<_>>()
            .into()
    })
}

/// Move an arrow array into an [`ArrayRef`].
pub fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
    std::sync::Arc::new(t) as ArrayRef
}

/// Downcast an arrow array to a concrete array type, without having to go via `Any`.
///
/// Used by the generated deserializers.
/// See also [`ArrowArrayDowncastRef`](https://docs.rs/re_arrow_util), which is the same thing
/// for code that wants an `ArrowError` instead of a [`DeserializationError`].
pub trait ArrayTryCast<'a>: 'a {
    /// Downcast to `T`, or fail with a datatype mismatch error.
    ///
    /// `expected` is only called on failure, to build the error message.
    /// It is a function pointer rather than a value so that we don't pay for
    /// building a [`DataType`] on the happy path.
    fn try_cast<T: Array + 'static>(
        self,
        expected: fn() -> DataType,
    ) -> DeserializationResult<&'a T>;
}

impl<'a> ArrayTryCast<'a> for &'a dyn Array {
    fn try_cast<T: Array + 'static>(
        self,
        expected: fn() -> DataType,
    ) -> DeserializationResult<&'a T> {
        self.as_any().downcast_ref::<T>().ok_or_else(|| {
            DeserializationError::datatype_mismatch(expected(), self.data_type().clone())
        })
    }
}

impl<'a> ArrayTryCast<'a> for &'a ArrayRef {
    fn try_cast<T: Array + 'static>(
        self,
        expected: fn() -> DataType,
    ) -> DeserializationResult<&'a T> {
        let array: &'a dyn Array = &**self;
        array.try_cast(expected)
    }
}

/// Fails if any entry of `array` is null.
///
/// Used by the generated deserializers for anything non-nullable:
/// there is no value to deserialize a null into.
///
/// Returns the array itself, so that it can be used in the middle of a chain.
pub fn err_on_nulls<'a>(
    array: &'a dyn Array,
    context: &'static str,
) -> DeserializationResult<&'a dyn Array> {
    if 0 < array.null_count() {
        return Err(DeserializationError::missing_data()).with_context(context);
    }
    Ok(array)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concatenate_byte_buffers() {
        assert!(concat_u8_buffers(&mut std::iter::empty()).is_empty());

        let first = ScalarBuffer::from(vec![1_u8, 2, 3]).slice(1, 2);
        let single = concat_u8_buffers(&mut std::iter::once(&first));
        assert_eq!(single.as_ptr(), first.as_ptr());
        assert_eq!(single.as_ref(), &[2, 3]);

        let second = ScalarBuffer::from(vec![4_u8, 5]);
        let third = ScalarBuffer::from(vec![6_u8]);
        assert_eq!(
            concat_u8_buffers(&mut [&first, &second, &third].into_iter()).as_ref(),
            &[2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn fixed_size_list_width_validation() {
        use arrow::array::{FixedSizeListArray, Int32Array};
        use arrow::datatypes::Field;

        fn expected() -> DataType {
            DataType::FixedSizeList(
                std::sync::Arc::new(Field::new_list_field(DataType::Int32, true)),
                2,
            )
        }
        let values = as_array_ref(Int32Array::from(vec![1, 2, 3, 4]));
        let array = FixedSizeListArray::new(
            std::sync::Arc::new(Field::new_list_field(DataType::Int32, true)),
            2,
            values.clone(),
            None,
        );
        assert_eq!(
            fixed_size_list_values(&array, 2, expected, "test")
                .unwrap()
                .len(),
            4
        );
        assert!(fixed_size_list_values(&array, 3, expected, "test").is_err());
        assert!(fixed_size_list_values(values.as_ref(), 2, expected, "test").is_err());
    }

    #[test]
    fn union_offsets_count_each_child() {
        assert!(dense_union_offsets(&[]).unwrap().is_empty());
        assert_eq!(
            dense_union_offsets(&[0, 2, 1, 2, 0, 127, 1, 127])
                .unwrap()
                .as_ref(),
            &[0, 0, 0, 1, 1, 0, 1, 1]
        );
        assert!(dense_union_offsets(&[-1]).is_err());
    }

    #[test]
    fn struct_fields_by_name() {
        use arrow::array::Int32Array;
        use arrow::datatypes::Field;

        let array = StructArray::from(vec![
            (
                std::sync::Arc::new(Field::new("b", DataType::Int32, false)),
                as_array_ref(Int32Array::from(vec![1])),
            ),
            (
                std::sync::Arc::new(Field::new("a", DataType::Int32, false)),
                as_array_ref(Int32Array::from(vec![2])),
            ),
            (
                std::sync::Arc::new(Field::new("b", DataType::Int32, false)),
                as_array_ref(Int32Array::from(vec![3])),
            ),
        ]);
        let fields = StructFields::new(&array, || DataType::Struct(Default::default()), "example");
        assert_eq!(
            fields
                .required("a")
                .unwrap()
                .as_any()
                .downcast_ref::<Int32Array>()
                .unwrap()
                .value(0),
            2
        );
        assert_eq!(
            fields
                .required("b")
                .unwrap()
                .as_any()
                .downcast_ref::<Int32Array>()
                .unwrap()
                .value(0),
            3
        );
        let err = re_error::format_ref(&fields.required("missing").unwrap_err());
        assert!(err.contains("example"), "{err}");
        assert!(err.contains("missing"), "{err}");
    }

    #[test]
    fn optional_strings_roundtrip() {
        let values = vec![
            Some(ArrowString::from("héllo")),
            None,
            Some(ArrowString::from("")),
            Some(ArrowString::from("世界")),
        ];
        let array = serialize_optional_strings(
            values
                .iter()
                .map(|value| value.as_ref().map(|value| value.as_str())),
            validity_from_bools(vec![true, false, true, true]),
        )
        .unwrap();
        let array = array.as_any().downcast_ref::<StringArray>().unwrap();
        assert_eq!(deserialize_strings(array).unwrap(), values);
    }

    #[test]
    fn deserialize_sliced_strings() {
        let values = vec![Some("héllo"), None, Some(""), Some("世界")];
        let array = StringArray::from(values.clone());

        // Slicing retains nonzero byte offsets into the original backing buffer.
        let sliced = array.slice(1, 3);
        let decoded = deserialize_strings(&sliced).unwrap();
        assert_eq!(
            decoded,
            values[1..]
                .iter()
                .map(|value| value.map(ArrowString::from))
                .collect::<Vec<_>>()
        );

        // Decoding must share the backing bytes rather than copy them.
        assert_eq!(
            decoded[2].as_ref().unwrap().as_bytes().as_ptr(),
            array.value(3).as_ptr()
        );
    }

    #[test]
    fn strings_roundtrip() {
        let values = vec![ArrowString::from("a"), ArrowString::from("β")];
        let array = serialize_strings(values.iter().map(|value| value.as_str()), None).unwrap();
        assert_eq!(
            deserialize_strings(array.as_any().downcast_ref().unwrap()).unwrap(),
            values.into_iter().map(Some).collect::<Vec<_>>()
        );
    }

    #[test]
    fn string_serializers_reject_mismatched_validity() {
        for length in [0, 2] {
            let nulls = Some(NullBuffer::new_null(length));
            assert!(serialize_strings(std::iter::once("a"), nulls.clone()).is_err());
            assert!(serialize_optional_strings(std::iter::once(Some("a")), nulls).is_err());
        }
    }

    #[test]
    fn validity_helpers() {
        assert!(validity_from_bools(vec![]).is_none());
        assert!(validity_from_bools(vec![true, true]).is_none());

        let validity = validity_from_bools(vec![true, false, true]).unwrap();
        assert_eq!(validity.iter().collect::<Vec<_>>(), [true, false, true]);
        assert!(expand_validity(None, 3).is_none());
        assert_eq!(
            expand_validity(Some(&validity), 2)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            [true, true, false, false, true, true],
        );
        assert_eq!(expand_validity(Some(&validity), 0).unwrap().len(), 0);
        let all_valid = NullBuffer::new_valid(2);
        assert_eq!(expand_validity(Some(&all_valid), 3).unwrap().len(), 6);
    }
}
