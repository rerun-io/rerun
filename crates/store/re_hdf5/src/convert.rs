//! HDF5 → Arrow conversion: dtype mapping, dataset reads, attribute mapping.
//!
//! Per-row representation: every dataset row is exactly one instance. The row's
//! inner Arrow value type encodes the per-row payload, and a component column is
//! always an outer `ListArray` with one element per row (offsets step by 1):
//!
//! - 0-D / 1-D dataset → each row is one scalar;
//! - 2-D `[N, K]` → each row is one `FixedSizeList<K>`;
//! - 3-D+ `[N, d1, …, dk]` → each row is one `List` blob of `d1*…*dk` values
//!   in row-major order (no shape metadata is emitted; consumers recover the
//!   shape via `list_datasets`).
//!
//! This keeps a dataset's standalone-component shape identical to its
//! struct-field shape ([`crate::Hdf5Config::use_structs`]): both come from
//! [`read_row_values`].

use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, FixedSizeListArray, Float32Array, Float64Array, Int8Array, Int16Array,
    Int32Array, Int64Array, ListArray, StringArray, StructArray, UInt8Array, UInt16Array,
    UInt32Array, UInt64Array,
};
use arrow::buffer::{OffsetBuffer, ScalarBuffer};
use arrow::datatypes::{Field, Fields};
use hdf5_pure::DType;
use re_sdk_types::{ComponentDescriptor, ComponentIdentifier};

use crate::config::IndexType;
use crate::error::Hdf5Error;
use crate::walk::{DatasetDesc, H5Path};

/// The element types this reader maps to Arrow: the 10 numeric widths + strings.
pub(crate) fn supported_dtype(dtype: &DType) -> bool {
    is_numeric_dtype(dtype) | matches!(dtype, DType::String | DType::VariableLengthString)
}

pub(crate) fn is_numeric_dtype(dtype: &DType) -> bool {
    matches!(
        dtype,
        DType::I8
            | DType::I16
            | DType::I32
            | DType::I64
            | DType::U8
            | DType::U16
            | DType::U32
            | DType::U64
            | DType::F32
            | DType::F64
    )
}

/// Element type of an HDF5 dataset, as exposed by [`crate::DatasetInfo::dtype`].
///
/// An owned mirror of the subset of `hdf5-pure`'s `DType` this reader maps, so the
/// young `hdf5-pure` type never leaks into the public API. `Display` yields
/// numpy-style names (`"uint8"`, `"float64"`, `"string"`) — deliberately not
/// `DType`'s `Display`, which prints short names (`"u8"`, `"f64"`).
///
/// `#[non_exhaustive]`: the mapped set is expected to grow (compound, enum, array,
/// …), so adding variants stays non-breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DatasetDtype {
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
    String,

    /// An element type this reader does not map (compound, enum, array, reference,
    /// opaque, …).
    Unsupported,
}

impl DatasetDtype {
    fn as_numpy_str(self) -> &'static str {
        match self {
            Self::Int8 => "int8",
            Self::Int16 => "int16",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::UInt8 => "uint8",
            Self::UInt16 => "uint16",
            Self::UInt32 => "uint32",
            Self::UInt64 => "uint64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::String => "string",
            Self::Unsupported => "unsupported",
        }
    }
}

impl std::fmt::Display for DatasetDtype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_numpy_str())
    }
}

impl From<&DType> for DatasetDtype {
    fn from(dtype: &DType) -> Self {
        match dtype {
            DType::I8 => Self::Int8,
            DType::I16 => Self::Int16,
            DType::I32 => Self::Int32,
            DType::I64 => Self::Int64,
            DType::U8 => Self::UInt8,
            DType::U16 => Self::UInt16,
            DType::U32 => Self::UInt32,
            DType::U64 => Self::UInt64,
            DType::F32 => Self::Float32,
            DType::F64 => Self::Float64,
            DType::String | DType::VariableLengthString => Self::String,

            //TODO(ab): support more of these?
            DType::Compound(_)
            | DType::Enum(_)
            | DType::Array(..)
            | DType::ObjectReference
            | DType::Other(_) => Self::Unsupported,
        }
    }
}

/// Read a dataset into its length-`N` per-row value array (see module docs)
/// and the matching `Field` (named after the dataset leaf).
///
/// The single core builder used by both the standalone and the struct-packed
/// emit paths.
pub(crate) fn read_row_values(
    file: &hdf5_pure::File,
    desc: &DatasetDesc,
) -> Result<(Field, ArrayRef), Hdf5Error> {
    re_tracing::profile_function!();

    let dataset = file
        .dataset(&desc.path.as_hdf5())
        .map_err(|source| Hdf5Error::read_dataset(&desc.path, source))?;
    let flat = read_flat_values(&dataset, &desc.dtype, &desc.path)?;

    #[expect(clippy::cast_possible_truncation)]
    let num_rows = desc.shape.first().copied().unwrap_or(1) as usize;

    let values: ArrayRef = match desc.shape.len() {
        // Each row is one scalar (a 0-D dataset yields a single row).
        0 | 1 => flat,

        // Each row is one fixed-size list of `K` (`K` lives in the Arrow type).
        2 => {
            let k = i32::try_from(desc.shape[1]).map_err(|_err| Hdf5Error::ListTooLong {
                length: desc.shape[1],
            })?;
            let item_field = Arc::new(Field::new("item", flat.data_type().clone(), true));
            Arc::new(FixedSizeListArray::try_new(item_field, k, flat, None)?)
        }

        // Each row is one blob of the row's `d1*…*dk` raw row-major values
        // (one instance per frame for image-like datasets, not one per pixel).
        _ => {
            #[expect(clippy::cast_possible_truncation)]
            let per_row = desc.shape[1..].iter().product::<u64>() as usize;
            let item_field = Arc::new(Field::new("item", flat.data_type().clone(), true));
            let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(per_row, num_rows));
            Arc::new(ListArray::try_new(item_field, offsets, flat, None)?)
        }
    };

    let field = Field::new(desc.name(), values.data_type().clone(), true);
    Ok((field, values))
}

/// Read a dataset's raw values into a flat (row-major) Arrow array.
fn read_flat_values(
    dataset: &hdf5_pure::Dataset<'_>,
    dtype: &DType,
    path: &H5Path,
) -> Result<ArrayRef, Hdf5Error> {
    let read_err = |source| Hdf5Error::read_dataset(path, source);
    Ok(match dtype {
        DType::I8 => Arc::new(Int8Array::from(dataset.read_i8().map_err(read_err)?)),
        DType::I16 => Arc::new(Int16Array::from(dataset.read_i16().map_err(read_err)?)),
        DType::I32 => Arc::new(Int32Array::from(dataset.read_i32().map_err(read_err)?)),
        DType::I64 => Arc::new(Int64Array::from(dataset.read_i64().map_err(read_err)?)),
        DType::U8 => Arc::new(UInt8Array::from(dataset.read_u8().map_err(read_err)?)),
        DType::U16 => Arc::new(UInt16Array::from(dataset.read_u16().map_err(read_err)?)),
        DType::U32 => Arc::new(UInt32Array::from(dataset.read_u32().map_err(read_err)?)),
        DType::U64 => Arc::new(UInt64Array::from(dataset.read_u64().map_err(read_err)?)),
        DType::F32 => Arc::new(Float32Array::from(dataset.read_f32().map_err(read_err)?)),
        DType::F64 => Arc::new(Float64Array::from(dataset.read_f64().map_err(read_err)?)),
        DType::String | DType::VariableLengthString => {
            Arc::new(StringArray::from(dataset.read_string().map_err(read_err)?))
        }
        unsupported => {
            // Planning filters unsupported dtypes out before any read.
            return Err(Hdf5Error::UnsupportedElementType {
                dtype: unsupported.to_string(),
            });
        }
    })
}

/// Standalone (non-struct) emit path: one component per dataset, one row per
/// dataset row.
pub(crate) fn read_dataset_to_list(
    file: &hdf5_pure::File,
    desc: &DatasetDesc,
) -> Result<(ComponentDescriptor, ListArray), Hdf5Error> {
    let (field, values) = read_row_values(file, desc)?;
    let list = wrap_one_per_row(field.with_name("item"), values)?;
    Ok((partial_descriptor(desc.name())?, list))
}

/// Struct emit path: assemble the per-dataset row values into one
/// `List<Struct>` component named `data`, one struct per row.
pub(crate) fn build_struct_component(
    columns: Vec<(Field, ArrayRef)>,
) -> Result<(ComponentDescriptor, ListArray), Hdf5Error> {
    let (fields, arrays): (Vec<_>, Vec<_>) = columns
        .into_iter()
        .map(|(field, array)| (Arc::new(field), array))
        .unzip();

    let struct_array = StructArray::try_new(Fields::from(fields), arrays, None)?;
    let item_field = Field::new("item", struct_array.data_type().clone(), true);
    let list = wrap_one_per_row(item_field, Arc::new(struct_array))?;
    Ok((ComponentDescriptor::partial("data"), list))
}

/// Wrap a per-row value array into the outer one-element-per-row `ListArray`.
fn wrap_one_per_row(item_field: Field, values: ArrayRef) -> Result<ListArray, Hdf5Error> {
    let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(1_usize, values.len()));
    Ok(ListArray::try_new(
        Arc::new(item_field),
        offsets,
        values,
        None,
    )?)
}

/// Read the 1-D index dataset and scale its values to nanoseconds.
///
/// Integer dtypes are cast to `i64` and then multiplied. Float dtypes are
/// multiplied in `f64` first and rounded to `i64` after, preserving sub-second
/// precision (`1.5` s → `1_500_000_000` ns) for float-seconds indices.
pub(crate) fn read_index_to_ns(
    file: &hdf5_pure::File,
    path: &H5Path,
    index_type: IndexType,
) -> Result<ScalarBuffer<i64>, Hdf5Error> {
    re_tracing::profile_function!();

    let read_err = |source| Hdf5Error::read_dataset(path, source);
    let dataset = file.dataset(&path.as_hdf5()).map_err(read_err)?;
    let multiplier = index_type.ns_multiplier();

    fn scale_ints<T: Into<i64>>(values: Vec<T>, multiplier: i64) -> Vec<i64> {
        values
            .into_iter()
            .map(|value| value.into() * multiplier)
            .collect()
    }

    #[expect(clippy::cast_possible_truncation)]
    fn scale_floats<T: Into<f64>>(values: Vec<T>, multiplier: i64) -> Vec<i64> {
        #[expect(clippy::cast_precision_loss)]
        let multiplier = multiplier as f64;
        values
            .into_iter()
            .map(|value| (value.into() * multiplier).round() as i64)
            .collect()
    }

    let values: Vec<i64> = match dataset.dtype().map_err(read_err)? {
        DType::I8 => scale_ints(dataset.read_i8().map_err(read_err)?, multiplier),
        DType::I16 => scale_ints(dataset.read_i16().map_err(read_err)?, multiplier),
        DType::I32 => scale_ints(dataset.read_i32().map_err(read_err)?, multiplier),
        DType::I64 => scale_ints(dataset.read_i64().map_err(read_err)?, multiplier),
        DType::U8 => scale_ints(dataset.read_u8().map_err(read_err)?, multiplier),
        DType::U16 => scale_ints(dataset.read_u16().map_err(read_err)?, multiplier),
        DType::U32 => scale_ints(dataset.read_u32().map_err(read_err)?, multiplier),
        #[expect(clippy::cast_possible_wrap)]
        DType::U64 => dataset
            .read_u64()
            .map_err(read_err)?
            .into_iter()
            .map(|value| value as i64 * multiplier)
            .collect(),
        DType::F32 => scale_floats(dataset.read_f32().map_err(read_err)?, multiplier),
        DType::F64 => scale_floats(dataset.read_f64().map_err(read_err)?, multiplier),
        non_numeric => {
            // Planning validates this before any read; kept as a real error for safety.
            return Err(Hdf5Error::IndexNotNumeric {
                path: path.to_string(),
                dtype: non_numeric.to_string(),
            });
        }
    };

    Ok(ScalarBuffer::from(values))
}

/// Map one HDF5 attribute to a single-row static component.
///
/// Scalar variants become a one-scalar `List<primitive>` row; array variants a
/// single row whose value is a `FixedSizeList<L>` — the same one-per-row rule
/// as datasets.
pub(crate) fn attr_to_component(
    name: &str,
    value: &hdf5_pure::AttrValue,
) -> Result<(ComponentDescriptor, ListArray), Hdf5Error> {
    use hdf5_pure::AttrValue;

    let values: ArrayRef = match value {
        AttrValue::F64(value) => Arc::new(Float64Array::from(vec![*value])),
        AttrValue::I32(value) => Arc::new(Int32Array::from(vec![*value])),
        AttrValue::I64(value) => Arc::new(Int64Array::from(vec![*value])),
        AttrValue::U32(value) => Arc::new(UInt32Array::from(vec![*value])),
        AttrValue::U64(value) => Arc::new(UInt64Array::from(vec![*value])),
        AttrValue::String(value) | AttrValue::AsciiString(value) => {
            Arc::new(StringArray::from(vec![value.as_str()]))
        }
        AttrValue::F64Array(values) => {
            one_row_fixed_size_list(Arc::new(Float64Array::from(values.clone())))?
        }
        AttrValue::I64Array(values) => {
            one_row_fixed_size_list(Arc::new(Int64Array::from(values.clone())))?
        }
        AttrValue::StringArray(values)
        | AttrValue::AsciiStringArray(values)
        | AttrValue::VarLenAsciiArray(values) => one_row_fixed_size_list(Arc::new(
            StringArray::from_iter_values(values.iter().map(String::as_str)),
        ))?,
    };

    let item_field = Field::new("item", values.data_type().clone(), true);
    let list = wrap_one_per_row(item_field, values)?;
    Ok((partial_descriptor(name)?, list))
}

/// Wrap a length-`L` array into a length-1 `FixedSizeList<L>` array (one row).
fn one_row_fixed_size_list(inner: ArrayRef) -> Result<ArrayRef, Hdf5Error> {
    let len = i32::try_from(inner.len()).map_err(|_err| Hdf5Error::ListTooLong {
        length: inner.len() as u64,
    })?;
    let item_field = Arc::new(Field::new("item", inner.data_type().clone(), true));
    Ok(Arc::new(FixedSizeListArray::try_new(
        item_field, len, inner, None,
    )?))
}

fn partial_descriptor(name: &str) -> Result<ComponentDescriptor, Hdf5Error> {
    let component = ComponentIdentifier::try_new(name)
        .map_err(|source| Hdf5Error::invalid_component_name(name, source))?;
    Ok(ComponentDescriptor::partial(component))
}
