//! The byte model behind cut decisions: an estimate of what a chunk's rows weigh, computed column
//! by column, without copying anything.

use std::collections::HashMap;

use arrow::array::{Array, AsArray as _, OffsetSizeTrait};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, UnionMode};

use re_byte_size::SizeBytes as _;
use re_chunk::Chunk;

/// Estimated cumulative row bytes of `chunk`: `cumulative[k]` is the row bytes of rows `[0, k)`,
/// so `cumulative[b] - cumulative[a]` is the row bytes of rows `[a, b)`, and `cumulative[0]` is
/// zero.
pub fn estimate_cumulative_row_bytes(chunk: &Chunk) -> Vec<u64> {
    let num_rows = chunk.num_rows();
    let mut rows = vec![16 + 8 * chunk.timelines().len() as u64; num_rows];
    let mut bitmaps = 0;
    for list_array in chunk.components().list_arrays() {
        let sizes = element_sizes(list_array);
        for (i, row) in rows.iter_mut().enumerate() {
            *row += sizes.bytes.get(i);
        }
        bitmaps += sizes.bitmaps;
    }

    let mut cumulative = running_sum(&rows);
    for (k, total) in cumulative.iter_mut().enumerate() {
        *total += bitmaps * bitmap_bytes(k);
    }
    cumulative
}

/// Estimated bytes of the elements of an array.
struct ElementSizes {
    /// Per element, excluding bitmaps.
    ///
    /// Fixed-width leaves, and nested types made only of them, are one width for every element,
    /// so no per-element storage is allocated for the bulk of the data (the bytes of a blob column,
    /// the coordinates of a point cloud). Only levels whose elements vary in size hold one entry
    /// each.
    bytes: ElementBytes,

    /// Bitmaps spanning the elements — validity, boolean values, those of same-length children —
    /// at one byte per eight elements each. A bitmap has no per-element size, so the parent
    /// charges it per range, see [`RangeSums::range`].
    bitmaps: u64,
}

enum ElementBytes {
    Uniform(u64),
    Varying(Vec<u64>),
}

impl ElementBytes {
    fn get(&self, i: usize) -> u64 {
        match self {
            Self::Uniform(width) => *width,
            Self::Varying(sizes) => sizes[i],
        }
    }
}

/// Bytes of element ranges of an array, for the offsets of a parent list: the running sum of its
/// [`ElementSizes`], built once, or its width, plus its bitmaps.
struct RangeSums {
    bytes: RangeBytes,
    bitmaps: u64,
}

enum RangeBytes {
    Uniform(u64),

    /// Entry `k` is the bytes of elements `[0, k]`, summed in place over the element sizes so
    /// the two tables are never allocated together.
    Cumulative(Vec<u64>),
}

impl RangeSums {
    fn of(sizes: ElementSizes) -> Self {
        Self {
            bytes: match sizes.bytes {
                ElementBytes::Uniform(width) => RangeBytes::Uniform(width),
                ElementBytes::Varying(mut sizes) => {
                    let mut total = 0;
                    for size in &mut sizes {
                        total += *size;
                        *size = total;
                    }
                    RangeBytes::Cumulative(sizes)
                }
            },
            bitmaps: sizes.bitmaps,
        }
    }

    /// Bytes of elements `[start, end)`.
    fn range(&self, start: usize, end: usize) -> u64 {
        let bytes = match &self.bytes {
            RangeBytes::Uniform(width) => (end - start) as u64 * width,
            RangeBytes::Cumulative(cumulative) => {
                let up_to = |k: usize| if k == 0 { 0 } else { cumulative[k - 1] };
                up_to(end) - up_to(start)
            }
        };
        bytes + self.bitmaps * bitmap_bytes(end - start)
    }
}

/// Estimated bytes of each element of `array`, recursing through nested types: the offset and the
/// values of a list element, the fields of a struct element, the chosen child of a union element,
/// down to the leaf buffers.
///
/// Note: Arrow's `get_slice_memory_size` slices one level only: a sliced list charges its whole
/// child buffer, so e.g. for a list of blobs every row would be charged every blob.
fn element_sizes(array: &(dyn Array + 'static)) -> ElementSizes {
    let len = array.len();
    let validity = u64::from(array.nulls().is_some());
    let (bytes, bitmaps) = match array.data_type() {
        DataType::List(_) => {
            let list = array.as_list::<i32>();
            let values = RangeSums::of(element_sizes(list.values().as_ref()));
            (list_element_bytes(list.offsets(), 4, &values), 0)
        }
        DataType::LargeList(_) => {
            let list = array.as_list::<i64>();
            let values = RangeSums::of(element_sizes(list.values().as_ref()));
            (list_element_bytes(list.offsets(), 8, &values), 0)
        }
        DataType::Map(_, _) => {
            let map = array.as_map();
            let entries = RangeSums::of(element_sizes(map.entries()));
            (list_element_bytes(map.offsets(), 4, &entries), 0)
        }
        DataType::FixedSizeList(_, size) => {
            let list = array.as_fixed_size_list();
            let size = *size as usize;
            let values = RangeSums::of(element_sizes(list.values().as_ref()));
            let bytes = match values.bytes {
                RangeBytes::Uniform(_) => ElementBytes::Uniform(values.range(0, size)),
                RangeBytes::Cumulative(_) => ElementBytes::Varying(
                    (0..len)
                        .map(|i| {
                            let start = list.value_offset(i) as usize;
                            values.range(start, start + size)
                        })
                        .collect(),
                ),
            };
            (bytes, 0)
        }
        DataType::Struct(_) => {
            let columns: Vec<ElementSizes> = array
                .as_struct()
                .columns()
                .iter()
                .map(|column| element_sizes(column.as_ref()))
                .collect();
            sum_elementwise(len, &columns, 0)
        }
        DataType::Union(fields, UnionMode::Sparse) => {
            // Slicing a sparse union slices its children with it: one type id per element, and
            // every child spans the whole range.
            let union = array.as_union();
            let children: Vec<ElementSizes> = fields
                .iter()
                .map(|(type_id, _)| element_sizes(union.child(type_id).as_ref()))
                .collect();
            sum_elementwise(len, &children, 1)
        }
        DataType::Union(fields, UnionMode::Dense) => {
            // Slicing a dense union keeps its children whole and slices the offsets pointing into
            // them: an element is charged the child element it points to. The children's bitmaps
            // span elements no range of the union covers, and are not charged.
            let union = array.as_union();
            let Some(offsets) = union.offsets() else {
                return ElementSizes {
                    bytes: spread(array.heap_size_bytes(), len),
                    bitmaps: 0,
                };
            };
            let children: HashMap<i8, ElementSizes> = fields
                .iter()
                .map(|(type_id, _)| (type_id, element_sizes(union.child(type_id).as_ref())))
                .collect();
            let bytes = std::iter::zip(union.type_ids(), offsets)
                .map(|(&type_id, &offset)| 5 + children[&type_id].bytes.get(offset as usize))
                .collect();
            (ElementBytes::Varying(bytes), 0)
        }
        DataType::Utf8 => (
            list_element_bytes(array.as_string::<i32>().offsets(), 4, &BYTES),
            0,
        ),
        DataType::LargeUtf8 => (
            list_element_bytes(array.as_string::<i64>().offsets(), 8, &BYTES),
            0,
        ),
        DataType::Binary => (
            list_element_bytes(array.as_binary::<i32>().offsets(), 4, &BYTES),
            0,
        ),
        DataType::LargeBinary => (
            list_element_bytes(array.as_binary::<i64>().offsets(), 8, &BYTES),
            0,
        ),
        DataType::FixedSizeBinary(width) => (ElementBytes::Uniform(*width as u64), 0),
        DataType::Boolean => (ElementBytes::Uniform(0), 1),
        DataType::Null => (ElementBytes::Uniform(0), 0),
        data_type => match data_type.primitive_width() {
            Some(width) => (ElementBytes::Uniform(width as u64), 0),
            // Dictionaries, views and run-end encoded arrays are not walked: their measured size
            // is spread evenly over their elements. A deep slice of one carries the whole
            // dictionary or data buffers, so every piece measures over by that shared part and
            // such chunks converge by re-cutting rather than in one pass; a shared part larger
            // than the slack band cuts down to single rows, each carrying a copy of it.
            None => (spread(array.heap_size_bytes(), len), 0),
        },
    };
    ElementSizes {
        bytes,
        bitmaps: bitmaps + validity,
    }
}

/// The values of a byte array: one byte each, no bitmap.
const BYTES: RangeSums = RangeSums {
    bytes: RangeBytes::Uniform(1),
    bitmaps: 0,
};

/// Bytes of one bitmap over `len` elements.
fn bitmap_bytes(len: usize) -> u64 {
    len.div_ceil(8) as u64
}

/// `[0, sizes[0], sizes[0] + sizes[1], …]`, one entry longer than `sizes`.
fn running_sum(sizes: &[u64]) -> Vec<u64> {
    let mut total = 0;
    std::iter::chain(
        std::iter::once(0),
        sizes.iter().map(|size| {
            total += size;
            total
        }),
    )
    .collect()
}

/// Element `i` of a list: `offset_width` for its offset plus its values, the child range
/// `[offsets[i], offsets[i + 1])`.
fn list_element_bytes<O: OffsetSizeTrait>(
    offsets: &OffsetBuffer<O>,
    offset_width: u64,
    values: &RangeSums,
) -> ElementBytes {
    ElementBytes::Varying(
        offsets
            .windows(2)
            .map(|window| offset_width + values.range(window[0].as_usize(), window[1].as_usize()))
            .collect(),
    )
}

/// `bytes` spread evenly over `len` elements.
fn spread(bytes: u64, len: usize) -> ElementBytes {
    ElementBytes::Uniform(bytes / len.max(1) as u64)
}

/// The elementwise sum of `parts`, each over `len` elements, plus `extra` bytes per element, and
/// the bitmaps of all parts: one width when every part is uniform, else one entry per element.
fn sum_elementwise(len: usize, parts: &[ElementSizes], extra: u64) -> (ElementBytes, u64) {
    let uniform = parts
        .iter()
        .map(|part| match &part.bytes {
            ElementBytes::Uniform(width) => Some(*width),
            ElementBytes::Varying(_) => None,
        })
        .sum::<Option<u64>>();
    let bytes = match uniform {
        Some(width) => ElementBytes::Uniform(width + extra),
        None => ElementBytes::Varying(
            (0..len)
                .map(|i| extra + parts.iter().map(|part| part.bytes.get(i)).sum::<u64>())
                .collect(),
        ),
    };
    (bytes, parts.iter().map(|part| part.bitmaps).sum())
}

#[cfg(test)]
mod tests {
    use arrow::array::{
        Array as _, ArrayRef, Int32Array, ListArray, MapArray, StructArray, UInt8Array, UnionArray,
    };
    use arrow::buffer::{OffsetBuffer, ScalarBuffer};
    use arrow::datatypes::{DataType, Field, Fields, UnionFields, UnionMode};
    use re_chunk::RowId;
    use re_log_types::Timeline;
    use re_types_core::ComponentDescriptor;

    use std::sync::Arc;

    use re_chunk::Span;

    use super::*;

    /// One `blob_len`-byte blob, as a one-element `List<UInt8>`.
    fn blob(seed: usize, blob_len: usize) -> ArrayRef {
        #[expect(clippy::cast_possible_truncation)] // wraps on purpose
        let values = UInt8Array::from_iter_values((0..blob_len).map(|b| (b + seed) as u8));
        Arc::new(ListArray::new(
            Arc::new(Field::new("item", DataType::UInt8, false)),
            OffsetBuffer::from_lengths([blob_len]),
            Arc::new(values),
            None,
        ))
    }

    /// `rows` rows of one `blob_len`-byte blob each, every blob wrapped by `wrap` into the row's
    /// component value.
    fn chunk_of(rows: usize, blob_len: usize, wrap: impl Fn(ArrayRef) -> ArrayRef) -> Chunk {
        let frame = Timeline::new_sequence("frame");
        let descriptor = ComponentDescriptor::partial("blob");
        let mut builder = Chunk::builder("entity");
        for i in 0..rows {
            builder = builder.with_row(
                RowId::from_u128(i as u128 + 1),
                [(frame, i64::try_from(i).unwrap())],
                [(descriptor.clone(), wrap(blob(i, blob_len)))],
            );
        }
        builder.build().unwrap()
    }

    /// A one-element union holding `value` under type id 0.
    fn union_of(value: ArrayRef, mode: UnionMode) -> ArrayRef {
        let fields = UnionFields::try_new(
            [0_i8],
            [Field::new("blob", value.data_type().clone(), false)],
        )
        .unwrap();
        let offsets = matches!(mode, UnionMode::Dense).then(|| ScalarBuffer::from(vec![0_i32]));
        Arc::new(
            UnionArray::try_new(fields, ScalarBuffer::from(vec![0_i8]), offsets, vec![value])
                .unwrap(),
        )
    }

    /// A one-entry map from an integer key to `value`.
    fn map_of(value: ArrayRef) -> ArrayRef {
        let entries = StructArray::new(
            Fields::from(vec![
                Field::new("key", DataType::Int32, false),
                Field::new("value", value.data_type().clone(), false),
            ]),
            vec![Arc::new(Int32Array::from(vec![0])), value],
            None,
        );
        Arc::new(MapArray::new(
            Arc::new(Field::new("entries", entries.data_type().clone(), false)),
            OffsetBuffer::from_lengths([1]),
            entries,
            None,
            false,
        ))
    }

    /// The per-row estimate charges a row its own blob only, and tracks the measured size of the
    /// deep slice it stands for to within a fixed cost, whatever the piece length.
    fn assert_rows_charged_their_own_blob(chunk: &Chunk, rows: usize, blob_len: usize) {
        let cumulative = estimate_cumulative_row_bytes(chunk);
        assert_eq!(cumulative.len(), rows + 1);

        let per_row = cumulative[1] - cumulative[0];
        assert!(
            (blob_len as u64..blob_len as u64 + 64).contains(&per_row),
            "a row is charged its blob plus offsets, got {per_row}"
        );

        let gap = |k: usize| {
            let measured = chunk
                .row_sliced_deep(Span::from_start_len(0, k))
                .total_size_bytes();
            measured as i128 - cumulative[k] as i128
        };
        let gaps: Vec<i128> = [1, 8, rows].into_iter().map(gap).collect();
        for &gap in &gaps {
            assert!(
                gap.unsigned_abs() < blob_len as u128 / 2,
                "estimate off by {gap} bytes, more than half a row"
            );
        }
    }

    #[test]
    fn cumulative_row_bytes_track_measured_slices() {
        let (rows, blob_len) = (32_usize, 4096_usize);
        let chunk = chunk_of(rows, blob_len, |blob| blob);
        assert_rows_charged_their_own_blob(&chunk, rows, blob_len);
    }

    /// A sliced dense union keeps its children whole; the estimate must not charge every row every
    /// blob (the `TensorBuffer` layout).
    #[test]
    fn dense_union_rows_are_charged_their_own_blob() {
        let (rows, blob_len) = (32_usize, 4096_usize);
        let chunk = chunk_of(rows, blob_len, |blob| union_of(blob, UnionMode::Dense));
        assert_rows_charged_their_own_blob(&chunk, rows, blob_len);
    }

    #[test]
    fn sparse_union_rows_are_charged_their_own_blob() {
        let (rows, blob_len) = (32_usize, 4096_usize);
        let chunk = chunk_of(rows, blob_len, |blob| union_of(blob, UnionMode::Sparse));
        assert_rows_charged_their_own_blob(&chunk, rows, blob_len);
    }

    #[test]
    fn map_rows_are_charged_their_own_blob() {
        let (rows, blob_len) = (32_usize, 4096_usize);
        let chunk = chunk_of(rows, blob_len, map_of);
        assert_rows_charged_their_own_blob(&chunk, rows, blob_len);
    }
}
