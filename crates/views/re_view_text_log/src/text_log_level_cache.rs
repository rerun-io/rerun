use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use re_viewer_context::Cache;

use crate::text_log_index_cache::TextLogQueryKey;

struct CachedLevels {
    levels: Arc<BTreeSet<String>>,
    last_used_generation: u64,
}

/// Distinct level labels keyed by timeline and ordered sources and component mappings.
/// Keys exclude the level filter so discovery is shared across filters.
/// Owned strings do not retain recording buffers.
/// Stale labels may remain until cache eviction or memory purging; discovery only adds labels.
#[derive(Default)]
pub struct TextLogLevelCache {
    entries: HashMap<TextLogQueryKey, CachedLevels>,
    generation: u64,
}

impl TextLogLevelCache {
    /// Accumulates discovered levels across spans independently of row filtering.
    /// `unfiltered_key` must be constructed with no level filter.
    pub fn extend_discovered_levels(
        &mut self,
        unfiltered_key: TextLogQueryKey,
        levels: BTreeSet<String>,
    ) -> Arc<BTreeSet<String>> {
        let entry = self
            .entries
            .entry(unfiltered_key)
            .or_insert_with(|| CachedLevels {
                levels: Arc::default(),
                last_used_generation: self.generation,
            });
        entry.last_used_generation = self.generation;
        if !levels.is_subset(&entry.levels) {
            Arc::make_mut(&mut entry.levels).extend(levels);
        }
        entry.levels.clone()
    }
}

impl Cache for TextLogLevelCache {
    fn name(&self) -> &'static str {
        "TextLogLevelCache"
    }

    fn begin_frame(&mut self) {
        self.entries
            .retain(|_, entry| entry.last_used_generation == self.generation);
        self.generation += 1;
    }

    fn purge_memory(&mut self) {
        self.entries.clear();
    }
}

impl re_byte_size::MemUsageTreeCapture for TextLogLevelCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        use re_byte_size::SizeBytes as _;
        re_byte_size::MemUsageTree::Bytes(
            self.entries
                .values()
                .map(|entry| entry.levels.total_size_bytes())
                .sum(),
        )
    }
}

/// Collects levels attached to text instances, excluding unused optional instances.
pub fn collect_levels(
    seen: &mut BTreeSet<String>,
    num_instances: usize,
    levels: Option<&[re_sdk_types::ArrowString]>,
) {
    for level in levels.into_iter().flatten().take(num_instances) {
        if !seen.contains(level.as_str()) {
            seen.insert(level.as_str().to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progressive_discovery_accumulates_within_its_configuration() {
        let key = TextLogQueryKey::for_test("configuration");
        let mut cache = TextLogLevelCache::default();
        cache.extend_discovered_levels(key, BTreeSet::from(["INFO".into()]));

        let levels = cache.extend_discovered_levels(key, BTreeSet::from(["WARN".into()]));
        let unchanged = cache.extend_discovered_levels(key, BTreeSet::new());

        assert_eq!(*levels, BTreeSet::from(["INFO".into(), "WARN".into()]));
        assert!(Arc::ptr_eq(&levels, &unchanged));
        let other = cache.extend_discovered_levels(
            TextLogQueryKey::for_test("other configuration"),
            BTreeSet::new(),
        );
        assert!(
            other.is_empty(),
            "new configurations must not inherit labels"
        );
    }
}
