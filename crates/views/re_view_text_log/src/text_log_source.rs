use re_chunk_index::RrdManifest;
use re_chunk_store::{Chunk, ChunkStore, ChunkTrackingMode, RangeQuery};
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, TimelineName};
use re_sdk_types::ComponentIdentifier;

/// Recording components used by one text-log instruction.
///
/// Blueprint mappings can redirect `text` and `level` to nonstandard recording components.
/// Coverage queries and store-event invalidation must track those mapped components.
#[derive(Clone, re_byte_size::SizeBytes)]
pub struct TextLogSourceKey {
    pub entity: EntityPath,
    pub timeline: TimelineName,
    pub text: ComponentIdentifier,
    pub level: ComponentIdentifier,
}

impl TextLogSourceKey {
    /// Reads temporal text batch ranges and counts intersecting `query` without retaining chunk payloads.
    pub fn text_ranges(
        &self,
        manifest: Option<&RrdManifest>,
        store: &ChunkStore,
        query: AbsoluteTimeRange,
    ) -> Vec<(AbsoluteTimeRange, u64)> {
        if let Some(manifest) = manifest {
            manifest
                .temporal_map()
                .get(&self.entity)
                .into_iter()
                .flat_map(|timelines| timelines.iter())
                .filter(|(timeline, _)| *timeline.name() == self.timeline)
                .filter_map(|(_, components)| components.get(&self.text))
                .flat_map(|chunks| chunks.values())
                .filter(|entry| {
                    entry.num_rows > 0
                        && !entry.time_range.is_empty()
                        && entry.time_range.intersects(query)
                })
                .map(|entry| (entry.time_range, entry.num_rows))
                .collect()
        } else {
            store
                .range_relevant_chunks(
                    ChunkTrackingMode::Ignore,
                    &RangeQuery::new(self.timeline, query),
                    &self.entity,
                    self.text,
                )
                .chunks
                .iter()
                .filter_map(|chunk| {
                    let count = chunk
                        .num_events_for_component(self.text)
                        .filter(|count| *count > 0)?;
                    let range = self.component_range(chunk, self.text)?;
                    range.intersects(query).then_some((range, count))
                })
                .collect()
        }
    }

    /// Time ranges whose text rows may have changed, given resident chunks whose rows were
    /// added or removed.
    ///
    /// A level change at time `t` can affect every later text row of this source.
    pub fn dirty_ranges<'a>(
        &'a self,
        changed: &'a [&'a Chunk],
        store: &ChunkStore,
    ) -> impl Iterator<Item = AbsoluteTimeRange> + 'a {
        let level_from = changed
            .iter()
            .filter_map(|chunk| self.component_range(chunk, self.level))
            .map(|range| range.min())
            .min();
        let level_dirty: Vec<_> = level_from.map_or_else(Vec::new, |start| {
            self.text_ranges(None, store, AbsoluteTimeRange::new(start, TimeInt::MAX))
                .into_iter()
                .map(|(range, _)| AbsoluteTimeRange::new(range.min().max(start), range.max()))
                .collect()
        });
        std::iter::chain(
            changed
                .iter()
                .filter_map(|chunk| self.component_range(chunk, self.text)),
            level_dirty,
        )
    }

    fn component_range(
        &self,
        chunk: &Chunk,
        component: ComponentIdentifier,
    ) -> Option<AbsoluteTimeRange> {
        if chunk.entity_path() != &self.entity {
            return None;
        }
        chunk
            .timelines()
            .get(&self.timeline)?
            .time_range_per_component(chunk.components())
            .get(&component)
            .copied()
            .filter(|range| !range.is_empty())
    }
}
