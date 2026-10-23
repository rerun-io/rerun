use std::collections::{BTreeMap, BTreeSet};

use itertools::izip;
use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimeInt, TimelineName};
use re_query::{range_zip_1x1, range_zip_1x2};
use re_sdk_types::ArrowString;
use re_view::ChunksWithComponent;

use crate::text_log_index_cache::IndexedRow;
use crate::text_log_level_cache::collect_levels;
use crate::visualizer_system::Entry;

/// Current query results used to populate display data, never retained by the row index.
pub struct TextLogQuery<'a> {
    pub entity_path: &'a EntityPath,
    pub timeline: TimelineName,
    pub texts: &'a ChunksWithComponent<'a>,
    pub levels: &'a ChunksWithComponent<'a>,
    pub colors: &'a ChunksWithComponent<'a>,
}

impl TextLogQuery<'_> {
    /// Writes matching instances into their table slots, preserving gaps for unavailable rows.
    /// `entries` must have one slot per row in `rows`, including rows from other sources.
    pub fn populate(
        &self,
        source: usize,
        rows: &[IndexedRow],
        entries: &mut [Option<Entry>],
        discovered: &mut BTreeSet<String>,
    ) {
        re_tracing::profile_function!();
        re_log::debug_assert_eq!(
            rows.len(),
            entries.len(),
            "each indexed row needs an entry slot"
        );

        let mut requested = BTreeMap::<_, Vec<_>>::new();
        for (slot, row) in rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.source == source)
        {
            requested
                .entry((row.time, row.row_id))
                .or_default()
                .push((row.instance, slot));
        }
        if requested.is_empty() {
            return;
        }
        let frames = range_zip_1x2(
            self.texts
                .iter()
                .filter(|chunk| !chunk.chunk.is_static())
                .flat_map(|chunk| {
                    izip!(
                        chunk.iter_component_indices(self.timeline),
                        chunk.iter_slices::<String>(),
                        chunk.iter_component_timepoints(),
                    )
                    .map(|(index, bodies, timepoint)| (index, (bodies, timepoint)))
                }),
            self.levels.slice::<String>(self.timeline),
            self.colors.slice::<u32>(self.timeline),
        );
        for (index, (bodies, timepoint), levels, colors) in frames {
            collect_levels(discovered, bodies.len(), levels.as_deref());
            let Some(instances) = requested.get(&index) else {
                continue;
            };
            for &(instance, slot) in instances {
                let Some(body) = bodies.get(instance) else {
                    continue;
                };
                let level = levels.as_ref().and_then(|levels| {
                    levels
                        .get(instance.min(levels.len().saturating_sub(1)))
                        .cloned()
                });
                let color = colors.as_ref().and_then(|colors| {
                    colors
                        .get(instance.min(colors.len().saturating_sub(1)))
                        .copied()
                        .map(Into::into)
                });
                entries[slot] = Some(Entry {
                    entity_path: self.entity_path.clone(),
                    timepoint: timepoint.clone(),
                    body: body.clone(),
                    level,
                    color,
                });
            }
        }
    }
}

/// A text row's identity and instance count, joined with its applicable level batch.
pub struct TextLogRowLevels {
    /// Time on the queried timeline.
    pub time: TimeInt,

    /// Identity of the text row, also used to order equal-time updates.
    pub row_id: RowId,

    /// Number of text instances to which the level batch applies (there can in theory be multiple texts on a single row).
    pub num_instances: usize,

    /// Unclamped level batch; absent or empty batches provide no level.
    pub levels: Option<Vec<ArrowString>>,
}

/// Joins text rows with the latest applicable level batch.
pub fn text_levels<'a>(
    timeline: TimelineName,
    texts: &'a ChunksWithComponent<'_>,
    levels: &'a ChunksWithComponent<'_>,
) -> impl Iterator<Item = TextLogRowLevels> + 'a {
    range_zip_1x1(
        texts
            .iter()
            .filter(|chunk| !chunk.chunk.is_static())
            .flat_map(move |chunk| {
                std::iter::zip(
                    chunk.iter_component_indices(timeline),
                    chunk
                        .chunk
                        .iter_component_offsets(chunk.component)
                        .map(|span| span.len),
                )
            }),
        levels.slice::<String>(timeline),
    )
    .map(|((time, row_id), num_instances, levels)| TextLogRowLevels {
        time,
        row_id,
        num_instances,
        levels,
    })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::BTreeSet;

    use re_chunk_store::{Chunk, Span};
    use re_log_types::{EntityPath, Timeline};
    use re_sdk_types::archetypes::TextLog;
    use re_sdk_types::components::TextLogLevel;

    use crate::text_log_index_cache::TextLogIndex;

    use super::*;

    fn components(
        chunks: &[Chunk],
        component: re_sdk_types::ComponentIdentifier,
    ) -> ChunksWithComponent<'_> {
        ChunksWithComponent {
            chunks: Cow::Borrowed(chunks),
            component,
        }
    }

    /// Population must match cached row IDs and instance indices, not chunk-local positions.
    /// Equal timestamps require row-ID matching, and a window starting mid-batch requires
    /// preserving the requested instance index rather than restarting at the first body.
    #[test]
    fn populate_matches_requested_row_and_instance_ids() {
        let timeline = Timeline::log_tick();
        let first = RowId::new();
        let second = RowId::new();
        let chunks = [Chunk::builder("logs")
            .with_archetype(
                first,
                [(timeline, 5)],
                &TextLog::update_fields().with_many_text(["a", "b"]),
            )
            .with_archetype(
                second,
                [(timeline, 5)],
                &TextLog::update_fields().with_text("c"),
            )
            .build()
            .unwrap()];
        let mut index = TextLogIndex::default();
        index.append(
            0,
            *timeline.name(),
            None,
            &components(&chunks, TextLog::descriptor_text().component),
            &components(&chunks, TextLog::descriptor_level().component),
            &mut BTreeSet::new(),
        );

        let split = [
            chunks[0].row_sliced_shallow(Span { start: 0, len: 1 }),
            chunks[0].row_sliced_shallow(Span { start: 1, len: 1 }),
        ];
        let rows = &index.rows[1..];

        let mut entries = vec![None; rows.len()];
        TextLogQuery {
            entity_path: &EntityPath::from("logs"),
            timeline: *timeline.name(),
            texts: &components(&split, TextLog::descriptor_text().component),
            levels: &components(&split, TextLog::descriptor_level().component),
            colors: &components(&split, TextLog::descriptor_color().component),
        }
        .populate(0, rows, &mut entries, &mut BTreeSet::new());

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.as_ref().unwrap().body.as_str())
                .collect::<Vec<_>>(),
            ["b", "c"]
        );
    }

    /// Unavailable rows must leave gaps rather than shift later entries into earlier slots.
    /// Matching time and row ID are insufficient: only the requested instruction's slots
    /// may be populated, even when another instruction references the same recording row.
    #[test]
    fn missing_rows_preserve_slots_and_instruction_identity() {
        let timeline = Timeline::log_tick();
        let row_id = RowId::new();
        let chunks = [Chunk::builder("logs")
            .with_archetype(
                row_id,
                [(timeline, 5)],
                &TextLog::update_fields().with_text("available"),
            )
            .build()
            .unwrap()];
        let rows = [
            IndexedRow {
                time: TimeInt::new_temporal(4),
                row_id: RowId::new(),
                source: 0,
                instance: 0,
            },
            IndexedRow {
                time: TimeInt::new_temporal(5),
                row_id,
                source: 1,
                instance: 0,
            },
            IndexedRow {
                time: TimeInt::new_temporal(5),
                row_id,
                source: 0,
                instance: 0,
            },
        ];
        let mut entries = vec![None; rows.len()];
        TextLogQuery {
            entity_path: &EntityPath::from("logs"),
            timeline: *timeline.name(),
            texts: &components(&chunks, TextLog::descriptor_text().component),
            levels: &components(&chunks, TextLog::descriptor_level().component),
            colors: &components(&chunks, TextLog::descriptor_color().component),
        }
        .populate(0, &rows, &mut entries, &mut BTreeSet::new());
        assert!(
            entries[0].is_none(),
            "unavailable rows must not shift later entries"
        );
        assert!(
            entries[1].is_none(),
            "an instruction must not populate another instruction's rows"
        );
        assert_eq!(entries[2].as_ref().unwrap().body.as_str(), "available");
    }

    fn query_index(
        chunks: &[Chunk],
        filter: Option<&BTreeSet<ArrowString>>,
    ) -> (TextLogIndex, Vec<Entry>, BTreeSet<String>) {
        let components = [
            TextLog::descriptor_text().component,
            TextLog::descriptor_level().component,
            TextLog::descriptor_color().component,
        ]
        .map(|component| ChunksWithComponent {
            chunks: Cow::Borrowed(chunks),
            component,
        });
        let mut index = TextLogIndex::default();
        let mut levels = BTreeSet::new();
        index.append(
            0,
            TimelineName::log_tick(),
            filter,
            &components[0],
            &components[1],
            &mut levels,
        );
        index.rows.sort_by_key(|row| row.time);
        let query = TextLogQuery {
            entity_path: &EntityPath::from("logs"),
            timeline: TimelineName::log_tick(),
            texts: &components[0],
            levels: &components[1],
            colors: &components[2],
        };
        let mut entries = vec![None; index.rows.len()];
        let mut display_levels = BTreeSet::new();
        query.populate(0, &index.rows, &mut entries, &mut display_levels);
        assert_eq!(display_levels, levels);
        let entries = entries.into_iter().map(Option::unwrap).collect();
        (index, entries, levels)
    }

    #[test]
    fn sparse_batches_preserve_filtering_and_instance_clamping() {
        let tick = Timeline::log_tick();
        let texts = ["a", "b", "c"];
        let levels = ["WARN", "INFO"];
        let chunk = Chunk::builder("logs")
            .with_archetype_auto_row(
                [(tick, 1)],
                &TextLog::update_fields()
                    .with_many_text(texts)
                    .with_many_level(levels),
            )
            .with_archetype_auto_row(
                [(tick, 3)],
                &TextLog::update_fields()
                    .with_text("d")
                    .with_many_level([] as [TextLogLevel; 0]),
            )
            .with_archetype_auto_row(
                [(tick, 4)],
                &TextLog::update_fields()
                    .with_text("e")
                    .with_many_level(["INFO", "UNUSED"]),
            )
            .build()
            .unwrap();
        let filter = BTreeSet::from([ArrowString::from("INFO")]);
        let chunks = [chunk];
        let (rows, entries, levels) = query_index(&chunks, Some(&filter));
        assert_eq!(rows.rows.len(), 4);
        assert_eq!(levels, BTreeSet::from(["INFO".into(), "WARN".into()]));
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.body.as_str())
                .collect::<Vec<_>>(),
            ["b", "c", "d", "e"]
        );
        assert_eq!(entries[0].level.as_ref().unwrap().as_str(), "INFO");
        assert_eq!(entries[1].level.as_ref().unwrap().as_str(), "INFO");
        assert!(entries[2].level.is_none());

        let (_, entries, _) = query_index(&chunks, Some(&BTreeSet::from(["WARN".into()])));
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.body.as_str())
                .collect::<Vec<_>>(),
            ["a", "d"],
            "clamped INFO instances must be excluded, while rows without a level remain",
        );
    }
}
