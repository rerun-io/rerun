use re_chunk::{ChunkId, Timeline};
use re_log_types::ResolvedTimeRange;

use crate::{ChunkStore, ChunkStoreEvent};

impl ChunkStore {
    /// Drop all events that are in the given range on the given timeline.
    ///
    /// Note that matching events will be dropped from all timelines they appear on.
    ///
    /// Static chunks are unaffected.
    ///
    /// Used to implement undo (erase the last event from the blueprint db).
    pub fn drop_time_range(
        &mut self,
        timeline: &Timeline,
        drop_range: ResolvedTimeRange,
    ) -> Vec<ChunkStoreEvent> {
        re_tracing::profile_function!();

        if drop_range.max() < drop_range.min() {
            return Default::default();
        }

        // Prepare the changes:

        let mut chunk_ids_to_drop = vec![];
        let mut new_chunks = vec![];

        for (chunk_id, chunk) in &self.chunks_per_chunk_id {
            let Some(time_column) = chunk.timelines().get(timeline) else {
                // static chunk, or chunk that doesn't overlap this timeline
                continue; // keep it
            };

            let chunk_range = time_column.time_range();

            if drop_range.contains_range(chunk_range) {
                // The whole chunk should be dropped!
                chunk_ids_to_drop.push(*chunk_id);
            } else if drop_range.intersects(chunk_range) {
                let chunk = chunk.sorted_by_timeline_if_unsorted(timeline);

                let num_rows = chunk.num_rows();

                // Get the sorted times:
                #[allow(clippy::unwrap_used)] // We already know the chunk has the timeline
                let time_column = chunk.timelines().get(timeline).unwrap();
                let times = time_column.times_raw();

                let drop_range_min = drop_range.min().as_i64();
                let drop_range_max = drop_range.max().as_i64();

                let min_idx = times.partition_point(|&time| time < drop_range_min);
                let max_idx = times.partition_point(|&time| time <= drop_range_max);

                {
                    // Sanity check:
                    debug_assert!(min_idx <= max_idx);
                    debug_assert!(drop_range_min <= times[min_idx]);
                    if 0 < min_idx {
                        debug_assert!(times[min_idx - 1] < drop_range_min);
                    }
                    if max_idx < num_rows {
                        debug_assert!(drop_range_max < times[max_idx]);
                        if 0 < max_idx {
                            debug_assert!(times[max_idx - 1] <= drop_range_max);
                        }
                    }
                }

                if min_idx < max_idx {
                    chunk_ids_to_drop.push(*chunk_id);
                    if 0 < min_idx {
                        new_chunks.push(chunk.row_sliced(0, min_idx).with_id(ChunkId::new()));
                    }
                    if max_idx < num_rows {
                        new_chunks.push(
                            chunk
                                .row_sliced(max_idx, num_rows - max_idx)
                                .with_id(ChunkId::new()),
                        );
                    }
                }
            }
        }

        // ------------------
        // Apply the changes:

        let generation = self.generation();
        let mut events: Vec<ChunkStoreEvent> = vec![];

        for chunk_id in chunk_ids_to_drop {
            for diff in self.remove_chunk(chunk_id) {
                events.push(ChunkStoreEvent {
                    store_id: self.id.clone(),
                    store_generation: generation.clone(),
                    event_id: self
                        .event_id
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    diff,
                });
            }
        }
        for mut chunk in new_chunks {
            chunk.sort_if_unsorted();
            #[allow(clippy::unwrap_used)] // The chunk came from the store, so it should be fine
            events.append(&mut self.insert_chunk(&chunk.into()).unwrap());
        }

        events
    }
}
