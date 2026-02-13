use re_byte_size::{BookkeepingBTreeMap, SizeBytes};
use re_log_types::TimeInt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CachedTransformValue<T> {
    /// Cache is invalidated, we don't know what state we're in.
    Invalidated,

    /// There's a transform at this time.
    Resident(T),

    /// The value has been cleared out at this time.
    Cleared,
}

impl<T: SizeBytes> SizeBytes for CachedTransformValue<T> {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Resident(item) => item.heap_size_bytes(),
            Self::Invalidated | Self::Cleared => 0,
        }
    }
}

pub fn add_invalidated_entry_if_not_already_cleared<T: PartialEq + SizeBytes>(
    transforms: &mut BookkeepingBTreeMap<TimeInt, CachedTransformValue<T>>,
    time: TimeInt,
) {
    transforms.mutate_entry(time, CachedTransformValue::Invalidated, |value| {
        if *value != CachedTransformValue::Cleared {
            *value = CachedTransformValue::Invalidated;
        }
    });
}
