use re_byte_size::{BookkeepingBTreeMap, SizeBytes};
use re_log_types::TimeInt;
use re_sdk_types::{ChunkId, RowId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CachedTransformValue<T> {
    /// Cache is invalidated, we don't know what state we're in.
    Invalidated {
        chunk_id: ChunkId, // TODO(RR-4439): rows are allowed to be distributed across several chunks.
        row_id: RowId,
    },

    /// There's a transform at this time.
    Resident { value: T, row_id: RowId },

    /// The value has been cleared out at this time.
    Cleared,
}

impl<T> CachedTransformValue<T> {
    pub fn row_id(&self) -> Option<RowId> {
        match self {
            Self::Resident { row_id, .. } | Self::Invalidated { row_id, .. } => Some(*row_id),
            Self::Cleared => None,
        }
    }
}

impl<T: SizeBytes> SizeBytes for CachedTransformValue<T> {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Resident { value, .. } => value.heap_size_bytes(),
            Self::Invalidated { .. } | Self::Cleared => 0,
        }
    }
}

pub fn add_invalidated_entry_if_not_already_cleared<T: PartialEq + SizeBytes>(
    transforms: &mut BookkeepingBTreeMap<TimeInt, CachedTransformValue<T>>,
    time: TimeInt,
    new_chunk_id: ChunkId,
    new_row_id: RowId,
) {
    transforms.mutate_entry(
        time,
        CachedTransformValue::Invalidated {
            chunk_id: new_chunk_id,
            row_id: new_row_id,
        },
        |value| {
            match value {
                CachedTransformValue::Invalidated { chunk_id, row_id } => {
                    // Update to the latest row id.
                    //
                    // There are two reasons why the row id may be equal:
                    // * there has been a compaction/split event and we have to update the chunk id now
                    // * the row is distributed across many chunks
                    //   TODO(RR-4439): this is not yet supported
                    //   TODO(RR-4441): we should at least warn if we hit that case. Surprisingly hard since we have to distinguish whether this is a new chunk or just a replacement.
                    if new_row_id >= *row_id {
                        *row_id = new_row_id;
                        *chunk_id = new_chunk_id;
                    }
                }
                CachedTransformValue::Resident { row_id, .. } => {
                    // If this is the same row id as before, we don't have to invalidate the cached value.
                    // However, if there's a new, higher row id, new (uncalculated) value wins over the previous one.
                    // TODO(RR-4439): to support rows distributed across several chunks, we need to invalidate.
                    //   TODO(RR-4441): we should at least warn if we hit that case. Surprisingly hard since we have to distinguish whether this is a new chunk or just a replacement.
                    if new_row_id > *row_id {
                        *value = CachedTransformValue::Invalidated {
                            chunk_id: new_chunk_id,
                            row_id: new_row_id,
                        };
                    }
                }
                CachedTransformValue::Cleared => {
                    // Always keep.
                }
            }
        },
    );
}
