use ahash::HashMap;
use re_byte_size::SizeBytes as _;
use re_log_types::EntityPathHash;
use re_sdk_types::blueprint::components::VisualizerInstructionId;
use re_viewer_context::{Cache, ViewId};

/// Key for [`NumSeriesLastSeen`].
///
/// Each field scopes the stored count. Changing any of them resets the baseline.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NumSeriesLastSeenKey {
    /// Separate state per time series view panel.
    pub view_id: ViewId,
    /// One entry per logged entity (each can have a different series width).
    pub entity_path_hash: EntityPathHash,
    /// Same entity can have multiple blueprint visualizer instructions.
    pub visualizer_instruction_id: VisualizerInstructionId,
    /// Non-identity mappings may cap the returned count; identity mappings do not.
    pub is_identity_mapping: bool,
    /// When limits are off, remapped scalars are not capped at [`crate::MAX_NUM_SERIES_FOR_REMAPPED_SCALARS`].
    pub limits_enabled: bool,
}

impl re_byte_size::SizeBytes for NumSeriesLastSeenKey {
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

/// Tracks the last seen scalar series count per query target for consistency checks.
///
/// The map only stores the previous value so we can warn when it changes between frames.
/// Entries are cleared when the viewer calls [`Cache::purge_memory`].
#[derive(Default)]
pub struct NumSeriesLastSeen {
    entries: HashMap<NumSeriesLastSeenKey, usize>,
}

impl NumSeriesLastSeen {
    /// Records `count` and returns it, calling `on_inconsistent` if it differs from the last value.
    pub fn record(
        &mut self,
        key: NumSeriesLastSeenKey,
        count: usize,
        on_inconsistent: impl FnOnce(usize, usize),
    ) -> usize {
        if let Some(&previous) = self.entries.get(&key)
            && previous != count
        {
            on_inconsistent(previous, count);
        }

        self.entries.insert(key, count);
        count
    }
}

impl Cache for NumSeriesLastSeen {
    fn name(&self) -> &'static str {
        "NumSeriesLastSeen"
    }

    fn purge_memory(&mut self) {
        self.entries.clear();
    }
}

impl re_byte_size::SizeBytes for NumSeriesLastSeen {
    fn heap_size_bytes(&self) -> u64 {
        self.entries.heap_size_bytes()
    }
}

impl re_byte_size::MemUsageTreeCapture for NumSeriesLastSeen {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.heap_size_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_log_types::EntityPath;

    fn test_key(entity: &str, instruction_index: usize) -> NumSeriesLastSeenKey {
        let entity_path = EntityPath::from(entity);
        NumSeriesLastSeenKey {
            view_id: ViewId::hashed_from_str("test-view"),
            entity_path_hash: entity_path.hash(),
            visualizer_instruction_id: VisualizerInstructionId::new_deterministic(
                &entity_path,
                instruction_index,
            ),
            is_identity_mapping: true,
            limits_enabled: true,
        }
    }

    #[test]
    fn record_changed_count_warns_with_previous_and_new() {
        let mut last_seen = NumSeriesLastSeen::default();
        let key = test_key("metrics", 0);
        let mut warnings = Vec::new();

        last_seen.record(key, 16, |previous, new_count| {
            warnings.push((previous, new_count));
        });
        last_seen.record(key, 4, |previous, new_count| {
            warnings.push((previous, new_count));
        });

        assert_eq!(warnings, [(16, 4)]);
    }

    #[test]
    fn record_tracks_keys_independently() {
        let mut last_seen = NumSeriesLastSeen::default();
        let key_a = test_key("metrics/a", 0);
        let key_b = test_key("metrics/b", 0);
        let mut warnings = Vec::new();

        last_seen.record(key_a, 16, |previous, new_count| {
            warnings.push((previous, new_count));
        });
        last_seen.record(key_b, 4, |previous, new_count| {
            warnings.push((previous, new_count));
        });
        last_seen.record(key_a, 4, |previous, new_count| {
            warnings.push((previous, new_count));
        });

        assert_eq!(warnings, [(16, 4)]);
    }
}
